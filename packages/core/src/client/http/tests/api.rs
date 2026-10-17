//! The gate and the meter, through a reading [`Client`] on a recording wire.

use std::cell::Cell;

use crate::client::http::limit::LIMIT;
use crate::client::http::transport::{EXCHANGE, INFO, Response, Transport, TransportError, info, user_request};
use crate::client::{Client, Network};
use crate::test_support::{Wire, block_on, ok};
use crate::utils::address::Address;

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

/// A reader on mainnet whose wire answers `answers` in turn, then `200 []`.
fn reader(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Client<Wire> {
    Client::reader(Wire::answering(answers).then(ok("[]")), Network::Mainnet, clock)
}

fn refused(retry_after: Option<&str>) -> Result<Response, TransportError> {
    let headers = retry_after.map(|seconds| vec![("Retry-After".to_owned(), seconds.to_owned())]).unwrap_or_default();
    Ok(Response { status: 429, headers, body: b"null".to_vec() })
}

fn read(kind: &str) -> String {
    format!(r#"{{"type":"{kind}"}}"#)
}

fn limited(ms: u64) -> Result<String, TransportError> {
    Err(TransportError::RateLimited { retry_after_ms: Some(ms) })
}

#[test]
fn every_post_goes_to_the_client_s_own_network() {
    at(0);
    for (network, url) in [
        (Network::Mainnet, "https://api.hyperliquid.xyz/info"),
        (Network::Testnet, "https://api.hyperliquid-testnet.xyz/info"),
    ] {
        let client = Client::reader(Wire::always("[]"), network, clock);
        block_on(client.post(INFO, read("meta"))).unwrap();
        let sent = client.http().sent();
        assert_eq!(sent[0].url, url);
        assert_eq!(sent[0].body, read("meta").into_bytes());
        assert_eq!(client.network(), network);
    }
}

#[test]
fn a_429_closes_the_gate_for_every_request_until_it_ends() {
    at(0);
    let client = reader([refused(None)]);
    assert_eq!(block_on(client.post(INFO, read("perpDexs"))), limited(2_000));
    assert_eq!(block_on(client.post(INFO, read("spotMeta"))), limited(2_000), "another read");
    assert_eq!(block_on(client.post(EXCHANGE, "{}".to_owned())), limited(2_000), "an action too");
    assert_eq!(client.http().sent().len(), 1, "only the first reached the wire");
    at(2_000);
    assert!(block_on(client.post(INFO, read("perpDexs"))).is_ok());
    assert_eq!(client.http().sent().len(), 2);
}

#[test]
fn a_longer_retry_after_from_the_venue_wins() {
    at(0);
    let client = reader([refused(Some("10"))]);
    assert_eq!(block_on(client.post(INFO, read("meta"))), limited(10_000));
    assert_eq!(client.spent().cooldown_ms, 10_000);
    assert_eq!(client.spent().refused, 1);
}

#[test]
fn any_other_status_is_the_venue_s_answer_and_is_still_counted() {
    at(0);
    let teapot = Response { status: 500, headers: Vec::new(), body: b"busy".to_vec() };
    let client = reader([Ok(teapot)]);
    assert_eq!(
        block_on(client.post(INFO, read("meta"))),
        Err(TransportError::Status { status: 500, body: "busy".to_owned() })
    );
    assert_eq!(client.spent().minute, 20);
    assert_eq!(client.spent().cooldown_ms, 0, "only a 429 closes the gate");
}

#[test]
fn no_answer_is_neither_counted_nor_struck() {
    at(0);
    let client = reader([Err(TransportError::Unreachable("reset".to_owned()))]);
    assert_eq!(block_on(client.post(EXCHANGE, "{}".to_owned())), Err(TransportError::Unreachable("reset".to_owned())));
    assert_eq!((client.spent().minute, client.spent().cooldown_ms), (0, 0));
}

#[test]
fn reads_hold_back_near_the_limit_and_actions_never_do() {
    at(0);
    let client = reader([]);
    for _ in 0..48 {
        block_on(client.post(INFO, read("metaAndAssetCtxs"))).unwrap();
    }
    let held = block_on(client.post(INFO, read("metaAndAssetCtxs")));
    assert!(matches!(held, Err(TransportError::RateLimited { retry_after_ms: Some(_) })), "{held:?}");
    assert_eq!(client.http().sent().len(), 48, "the held read never went out");
    assert!(block_on(client.post(EXCHANGE, r#"{"action":{"type":"cancel","cancels":[]}}"#.to_owned())).is_ok());
    assert_eq!(client.http().sent().len(), 49);
}

#[test]
fn what_went_out_is_weighed_by_kind() {
    at(0);
    let fills = format!("[{}]", vec!["{}"; 45].join(","));
    let orders = format!(r#"{{"action":{{"type":"order","orders":[{}]}},"nonce":1}}"#, vec!["{}"; 45].join(","));
    let client = reader([ok("{}"), ok(&fills), ok("{}")]);
    block_on(client.post(INFO, read("l2Book"))).unwrap();
    block_on(client.post(INFO, read("userFills"))).unwrap();
    block_on(client.post(EXCHANGE, orders)).unwrap();
    let spent = client.spent();
    assert_eq!(spent.limit, LIMIT);
    assert_eq!(spent.minute, 2 + 22 + 2);
    let kinds: Vec<(&str, u32)> = spent.kinds.iter().map(|kind| (kind.name.as_str(), kind.weight)).collect();
    assert_eq!(kinds, [("userFills", 22), ("exchange", 2), ("l2Book", 2)]);
}

#[test]
fn a_module_s_read_goes_through_the_gate_unchanged() {
    at(0);
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    let client = Client::reader(Wire::answering([ok(r#"{"role":"user"}"#)]), Network::Testnet, clock);
    let answer: serde_json::Value = block_on(info(&client, user_request("userRole", user))).unwrap();
    assert_eq!(answer["role"], "user");
    assert_eq!(client.spent().minute, 60);
}
