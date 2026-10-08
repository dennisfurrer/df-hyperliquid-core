use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::*;
use crate::{
    address::Address,
    client::{EXCHANGE, Method, Request, Response, user_request},
    limit::LIMIT,
};

thread_local! {
    static NOW: Cell<u64> = const { Cell::new(0) };
}

/// Each test runs on its own thread, so each has its own clock.
fn clock() -> u64 {
    NOW.with(Cell::get)
}

fn at(ms: u64) {
    NOW.with(|now| now.set(ms));
}

/// Answers from a queue, `200 []` once it runs dry, and keeps every request.
#[derive(Default)]
struct Wire {
    answers: RefCell<VecDeque<Result<Response, TransportError>>>,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    fn answering(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Self {
        Wire { answers: RefCell::new(answers.into_iter().collect()), sent: RefCell::default() }
    }

    fn sent(&self) -> usize {
        self.sent.borrow().len()
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(self.answers.borrow_mut().pop_front().unwrap_or_else(|| Ok(ok("[]"))))
    }
}

fn ok(text: &str) -> Response {
    Response { status: 200, headers: Vec::new(), body: text.as_bytes().to_vec() }
}

fn refused(retry_after: Option<&str>) -> Result<Response, TransportError> {
    let headers = retry_after.map(|seconds| vec![("Retry-After".to_owned(), seconds.to_owned())]).unwrap_or_default();
    Ok(Response { status: 429, headers, body: b"null".to_vec() })
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

fn read(kind: &str) -> String {
    format!(r#"{{"type":"{kind}"}}"#)
}

fn limited(ms: u64) -> Result<String, TransportError> {
    Err(TransportError::RateLimited { retry_after_ms: Some(ms) })
}

#[test]
fn every_post_goes_to_the_api_s_own_network() {
    at(0);
    for (network, url) in [
        (Network::Mainnet, "https://api.hyperliquid.xyz/info"),
        (Network::Testnet, "https://api.hyperliquid-testnet.xyz/info"),
    ] {
        let api = Api::new(Wire::default(), network, clock);
        block_on(api.post(INFO, read("meta"))).unwrap();
        let sent = &api.http().sent.borrow()[0];
        assert_eq!((sent.method, sent.url.as_str()), (Method::Post, url));
        assert_eq!(sent.header("content-type"), Some("application/json"));
        assert_eq!(sent.body, read("meta").into_bytes());
        assert_eq!(api.network(), network);
    }
}

#[test]
fn a_429_closes_the_gate_for_every_request_until_it_ends() {
    at(0);
    let api = Api::new(Wire::answering([refused(None)]), Network::Mainnet, clock);
    assert_eq!(block_on(api.post(INFO, read("perpDexs"))), limited(2_000));
    assert_eq!(block_on(api.post(INFO, read("spotMeta"))), limited(2_000), "another read");
    assert_eq!(block_on(api.post(EXCHANGE, "{}".to_owned())), limited(2_000), "an action too");
    assert_eq!(api.http().sent(), 1, "only the first reached the wire");
    at(2_000);
    assert!(block_on(api.post(INFO, read("perpDexs"))).is_ok());
    assert_eq!(api.http().sent(), 2);
}

#[test]
fn a_longer_retry_after_from_the_venue_wins() {
    at(0);
    let api = Api::new(Wire::answering([refused(Some("10"))]), Network::Mainnet, clock);
    assert_eq!(block_on(api.post(INFO, read("meta"))), limited(10_000));
    assert_eq!(api.spent().cooldown_ms, 10_000);
    assert_eq!(api.spent().refused, 1);
}

#[test]
fn any_other_status_is_the_venue_s_answer_and_is_still_counted() {
    at(0);
    let teapot = Response { status: 500, headers: Vec::new(), body: b"busy".to_vec() };
    let api = Api::new(Wire::answering([Ok(teapot)]), Network::Mainnet, clock);
    assert_eq!(
        block_on(api.post(INFO, read("meta"))),
        Err(TransportError::Status { status: 500, body: "busy".to_owned() })
    );
    assert_eq!(api.spent().minute, 20);
    assert_eq!(api.spent().cooldown_ms, 0, "only a 429 closes the gate");
}

#[test]
fn no_answer_is_neither_counted_nor_struck() {
    at(0);
    let api =
        Api::new(Wire::answering([Err(TransportError::Unreachable("reset".to_owned()))]), Network::Mainnet, clock);
    assert_eq!(block_on(api.post(EXCHANGE, "{}".to_owned())), Err(TransportError::Unreachable("reset".to_owned())));
    assert_eq!((api.spent().minute, api.spent().cooldown_ms), (0, 0));
}

#[test]
fn reads_hold_back_near_the_limit_and_actions_never_do() {
    at(0);
    let api = Api::new(Wire::default(), Network::Mainnet, clock);
    for _ in 0..48 {
        block_on(api.post(INFO, read("metaAndAssetCtxs"))).unwrap();
    }
    let held = block_on(api.post(INFO, read("metaAndAssetCtxs")));
    assert!(matches!(held, Err(TransportError::RateLimited { retry_after_ms: Some(_) })), "{held:?}");
    assert_eq!(api.http().sent(), 48, "the held read never went out");
    assert!(block_on(api.post(EXCHANGE, r#"{"action":{"type":"cancel","cancels":[]}}"#.to_owned())).is_ok());
    assert_eq!(api.http().sent(), 49);
}

#[test]
fn what_went_out_is_weighed_by_kind() {
    at(0);
    let fills = format!("[{}]", vec!["{}"; 45].join(","));
    let orders = format!(r#"{{"action":{{"type":"order","orders":[{}]}},"nonce":1}}"#, vec!["{}"; 45].join(","));
    let api = Api::new(Wire::answering([Ok(ok("{}")), Ok(ok(&fills)), Ok(ok("{}"))]), Network::Mainnet, clock);
    block_on(api.post(INFO, read("l2Book"))).unwrap();
    block_on(api.post(INFO, read("userFills"))).unwrap();
    block_on(api.post(EXCHANGE, orders)).unwrap();
    let spent = api.spent();
    assert_eq!(spent.limit, LIMIT);
    assert_eq!(spent.minute, 2 + 22 + 2);
    let kinds: Vec<(&str, u32)> = spent.kinds.iter().map(|kind| (kind.name.as_str(), kind.weight)).collect();
    assert_eq!(kinds, [("userFills", 22), ("exchange", 2), ("l2Book", 2)]);
}

#[test]
fn a_crate_s_read_goes_through_the_gate_unchanged() {
    at(0);
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    let api = Api::new(Wire::answering([Ok(ok(r#"{"role":"user"}"#))]), Network::Testnet, clock);
    let answer: serde_json::Value = block_on(crate::client::info(&api, user_request("userRole", user))).unwrap();
    assert_eq!(answer["role"], "user");
    assert_eq!(api.spent().minute, 60);
}
