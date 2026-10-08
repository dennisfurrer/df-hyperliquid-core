//! The client over a recorded wire: what goes out, with the key only in its header, and what
//! comes back, with the key cut out of anything the upstream said. Refusals are in the shape
//! Hydromancer documents (`{"error": "..."}`, docs.hydromancer.xyz REST API, read 2026-10-08).

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{Address, Http, Json, Method, Request, Response, TransportError};

use super::*;
use crate::{dashboard, snapshot};

/// Characters a URL must escape, so the socket's spelling of the key differs from the header's.
const SECRET: &str = "hm_live/the+actual=secret";
const ESCAPED: &str = "hm_live%2Fthe%2Bactual%3Dsecret";

const BUILDER: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";

/// Answers from a queue, `200 {}` once it runs dry, and keeps every request.
#[derive(Default)]
struct Wire {
    answers: RefCell<VecDeque<Result<Response, TransportError>>>,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    fn answering(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Self {
        Wire { answers: RefCell::new(answers.into_iter().collect()), sent: RefCell::default() }
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(self.answers.borrow_mut().pop_front().unwrap_or_else(|| Ok(answer(200, "{}"))))
    }
}

fn answer(status: u16, body: &str) -> Response {
    Response { status, headers: Vec::new(), body: body.as_bytes().to_vec() }
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

fn client(wire: Wire) -> Hydromancer<Wire> {
    Hydromancer::mainnet(wire, Key::new(SECRET).unwrap())
}

fn builder() -> Address {
    BUILDER.parse().unwrap()
}

#[test]
fn a_key_is_trimmed_and_an_empty_or_spaced_one_is_refused() {
    assert!(Key::new("  hm_live_abc \n").is_ok());
    for bad in ["", "   ", "hm live", "hm_\u{7f}", "hm_\tlive", "clé"] {
        assert!(Key::new(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_key_is_never_printed() {
    let key = Key::new(SECRET).unwrap();
    let client = client(Wire::default());
    for shown in [format!("{key:?}"), format!("{client:?}")] {
        assert!(!shown.contains(SECRET) && !shown.contains(ESCAPED), "{shown}");
        assert!(shown.contains("redacted"), "{shown}");
    }
}

#[test]
fn a_dashboard_read_is_a_get_with_the_key_in_its_header_alone() {
    let client = client(Wire::default());
    let request = client.request(&dashboard::stats(Some(builder())));
    assert_eq!(request.method, Method::Get);
    assert_eq!(request.url, format!("https://api.hydromancer.xyz/dashboard/outcome-builder-stats?builder={BUILDER}"));
    assert_eq!(request.header("authorization"), Some(format!("Bearer {SECRET}").as_str()));
    assert!(request.body.is_empty());
    assert!(!request.url.contains(SECRET) && !request.url.contains(ESCAPED));
}

#[test]
fn an_info_read_posts_json_with_the_key_in_its_header() {
    let client = client(Wire::default());
    let request = client.request(&snapshot::l4_book(&["BTC"]));
    assert_eq!(request.method, Method::Post);
    assert_eq!(request.url, "https://api.hydromancer.xyz/info");
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(request.header("Authorization"), Some(format!("Bearer {SECRET}").as_str()));
    assert_eq!(String::from_utf8(request.body).unwrap(), r#"{"type":"l4Book","coins":["BTC"]}"#);
}

#[test]
fn a_typed_read_goes_out_once_and_comes_back_typed() {
    let stats = include_str!("fixtures/dashboard_stats.json");
    let client = client(Wire::answering([Ok(answer(200, stats))]));
    let read = block_on(client.builder_stats(Some(builder()))).unwrap();
    assert_eq!(read.fills, 1_347_844);
    assert_eq!(client.http().sent.borrow().len(), 1);
}

#[test]
fn a_refusal_keeps_the_provider_s_words_and_loses_the_key() {
    let said = format!(r#"{{"error": "Invalid API key {SECRET} ({ESCAPED})"}}"#);
    let client = client(Wire::answering([Ok(answer(401, &said))]));
    let error = block_on(client.builder_stats(Some(builder()))).unwrap_err();
    assert_eq!(error.status(), Some(401));
    let shown = error.to_string();
    assert!(shown.contains("Invalid API key <redacted> (<redacted>)"), "{shown}");
    assert!(shown.contains("outcome-builder-stats"), "{shown}");
    assert!(!shown.contains(SECRET) && !shown.contains(ESCAPED), "{shown}");
    assert!(!error.is_retryable(), "a bad key stays bad");
}

#[test]
fn a_body_that_is_not_json_is_quoted_as_it_came_and_cut_short() {
    let page = format!("<html>502 Bad Gateway{}</html>", " ".repeat(1_000));
    let client = client(Wire::answering([Ok(answer(502, &page))]));
    let error = block_on(client.builder_stats(None)).unwrap_err();
    let Error::Status { status, said, .. } = &error else { panic!("{error:?}") };
    assert_eq!(*status, 502);
    assert!(said.starts_with("<html>502 Bad Gateway") && said.chars().count() <= 300, "{said}");
    assert!(error.is_retryable());
}

#[test]
fn no_answer_at_all_is_unreachable_and_never_carries_the_key() {
    let failed = format!("error sending request for url (wss://api.hydromancer.xyz/ws?token={ESCAPED}): {SECRET}");
    let client = client(Wire::answering([Err(TransportError::Unreachable(failed))]));
    let error = block_on(client.builder_stats(None)).unwrap_err();
    assert!(matches!(error, Error::Unreachable(_)), "{error:?}");
    let shown = error.to_string();
    assert!(!shown.contains(SECRET) && !shown.contains(ESCAPED), "{shown}");
    assert!(error.is_retryable());
}

#[test]
fn an_answer_that_does_not_read_says_where() {
    let client = client(Wire::answering([Ok(answer(200, r#"{"builder": 7}"#))]));
    let error = block_on(client.builder_stats(None)).unwrap_err();
    assert!(matches!(&error, Error::Unreadable { name, .. } if name == "outcome-builder-stats"), "{error:?}");
    assert!(!error.is_retryable(), "the same answer would not read the second time");
}

#[test]
fn only_an_overloaded_or_unreachable_provider_is_worth_another_try() {
    let status = |status: u16| Error::Status { name: "l4Book".to_owned(), status, said: String::new() };
    for worth in [429_u16, 500, 502, 503, 504, 599] {
        assert!(status(worth).is_retryable(), "{worth}");
    }
    for not in [400_u16, 401, 403, 404, 422] {
        assert!(!status(not).is_retryable(), "{not}");
    }
}

#[test]
fn a_raw_route_is_sent_as_typed_and_its_floats_keep_their_digits() {
    let client = client(Wire::answering([Ok(answer(200, r#"{"apr":0.037569521048262375,"fills":[1,2]}"#))]));
    let call = Call::dashboard("outcome-builder-recent-fills", &format!("builder={BUILDER}&venue=out")).unwrap();
    let read = block_on(client.raw(&call)).unwrap();
    assert_eq!(read.get("apr"), Some(&Json::Number("0.037569521048262375".to_owned())));
    assert_eq!(
        client.http().sent.borrow()[0].url,
        format!("https://api.hydromancer.xyz/dashboard/outcome-builder-recent-fills?builder={BUILDER}&venue=out")
    );
}

#[test]
fn a_raw_route_that_would_leave_the_dashboard_is_refused() {
    for bad in ["", "../info", "outcome-builder-stats?x=1", "a/b", "a#b", "a b", "%2e%2e"] {
        assert!(Call::dashboard(bad, "").is_err(), "{bad:?}");
    }
    for bad in ["builder=0x1 2", "builder=0x1#frag", "a=\n"] {
        assert!(Call::dashboard("outcome-builder-stats", bad).is_err(), "{bad:?}");
    }
    assert!(Call::dashboard("outcome_builder-Stats2", "").is_ok());
}

#[test]
fn a_raw_info_read_is_named_by_its_type() {
    let body = Json::parse(r#"{"type":"perpDexStatus","dex":"xyz"}"#).unwrap();
    let call = Call::info(&body);
    assert_eq!(call.name(), "perpDexStatus");
    assert_eq!(call.url(), "https://api.hydromancer.xyz/info");
    assert_eq!(call.body(), r#"{"type":"perpDexStatus","dex":"xyz"}"#);
}

#[test]
fn the_key_is_scrubbed_in_both_its_spellings() {
    let client = client(Wire::default());
    let said = format!("{SECRET} and {ESCAPED} and nothing else");
    assert_eq!(client.scrub(&said), "<redacted> and <redacted> and nothing else");
}

#[test]
fn hydromancer_is_mainnet_only_by_construction() {
    assert_eq!(API, "https://api.hydromancer.xyz");
    assert_eq!(SOCKET, "wss://api.hydromancer.xyz/ws");
    let request = client(Wire::default()).request(&dashboard::categories(None));
    assert!(request.url.starts_with("https://api.hydromancer.xyz/"), "{}", request.url);
}
