//! Through a recording wire, so nothing leaves the process. The signatures are
//! checked against independent vectors: `vectors/l1.json` (@nktkas/hyperliquid
//! 0.24.3) and `vectors/nktkas-0.33.2.json`, both on testnet.

use std::{
    cell::{Cell, RefCell},
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::account::{LocalKey, SignError, Signature, Signer};
use crate::client::{ApiError, Http, Network, Request, Response, TransportError};
use crate::utils::sign::{Domain, NonceKey, TypedStruct, UserAction, UserSigned};
use crate::utils::{Address, Json};

use super::*;

const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));
const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
const RESTING: &str = r#"{"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":77}}]}}}"#;
const OK: &str = r#"{"status":"ok","response":{"type":"default"}}"#;

thread_local! {
    static NOW: Cell<u64> = const { Cell::new(1_790_000_000_000) };
}

fn clock() -> u64 {
    NOW.with(Cell::get)
}

/// Answers every request with `answer` and keeps them all.
struct Wire {
    answer: String,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    fn answering(answer: &str) -> Self {
        Wire { answer: answer.to_owned(), sent: RefCell::new(Vec::new()) }
    }

    fn bodies(&self) -> Vec<(String, String)> {
        self.sent
            .borrow()
            .iter()
            .map(|request| (request.url.clone(), String::from_utf8_lossy(&request.body).into_owned()))
            .collect()
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(Ok(Response { status: 200, headers: Vec::new(), body: self.answer.clone().into_bytes() }))
    }
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

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap()
}

/// Some files write `r` and `s` without leading zeros; the body has 32 bytes.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

fn signature_json(expected: &Json) -> String {
    format!(
        r#""signature":{{"r":"{}","s":"{}","v":{}}}"#,
        word(text(expected, "r")),
        word(text(expected, "s")),
        expected.get("v").and_then(Json::as_u64).unwrap()
    )
}

#[test]
fn an_l1_action_is_signed_for_the_client_s_network_and_posted_once() {
    let file = parse(L1);
    let case = file
        .get("cases")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .find(|case| text(case, "name") == "order_vault_testnet")
        .unwrap();
    let vault: Address = text(case, "vaultAddress").parse().unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let client = Client::new(Wire::answering(RESTING), Network::Testnet, clock, key).acting_for(vault);
    let action = case.get("action").unwrap().to_action().unwrap();
    let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
    let accepted = block_on(client.send_at(&action, nonce)).unwrap();
    assert_eq!(accepted.kind, "order");
    let sent = client.http().bodies();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "https://api.hyperliquid-testnet.xyz/exchange");
    assert!(sent[0].1.contains(&signature_json(case.get("signature").unwrap())), "{}", sent[0].1);
    assert!(sent[0].1.contains(&format!(r#""vaultAddress":"{}""#, vault.to_hex())), "{}", sent[0].1);
}

/// `usdClassTransfer`, user-signed: the chain names, the digest and the
/// signature follow the client's network.
struct ClassTransfer(u64);

impl UserAction for ClassTransfer {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("usdClassTransfer", "HyperliquidTransaction:UsdClassTransfer", NonceKey::Nonce, self.0)
            .field("amount", crate::utils::sign::Field::String("12.5"))
            .field("toPerp", crate::utils::sign::Field::Bool(true))
    }
}

#[test]
fn a_user_signed_action_is_signed_under_the_network_s_chain() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let case = file
        .get("userSigned")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .find(|case| text(case, "name") == "usdClassTransfer" && case.get("isTestnet") == Some(&Json::Bool(true)))
        .unwrap();
    let client = Client::new(Wire::answering(OK), Network::Testnet, clock, key);
    block_on(client.send_user(&ClassTransfer(nonce))).unwrap();
    let sent = client.http().bodies();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].1.starts_with(&format!(r#"{{"action":{}"#, case.get("action").unwrap().compact())),
        "{}",
        sent[0].1
    );
    assert!(sent[0].1.contains(&signature_json(case.get("signature").unwrap())), "{}", sent[0].1);
}

#[test]
fn a_refusal_is_the_venue_s_words_and_is_never_sent_again() {
    let refused = r#"{"status":"err","response":"Insufficient margin to place order."}"#;
    let client =
        Client::new(Wire::answering(refused), Network::Mainnet, clock, LocalKey::from_bytes(&[7; 32]).unwrap());
    let action = parse(r#"{"type":"noop"}"#).to_action().unwrap();
    let error = block_on(client.send(&action)).unwrap_err();
    assert_eq!(error, SendError::Api(ApiError::Refused("Insufficient margin to place order.".to_owned())));
    assert_eq!(client.http().bodies().len(), 1);
}

#[test]
fn every_crate_s_reads_go_through_the_client_and_its_gate() {
    let client = Client::reader(Wire::answering(r#"{"BTC":"80459.5"}"#), Network::Mainnet, clock);
    let mids = block_on(crate::markets::mids::all_mids(&client, crate::markets::Dex::Main)).unwrap();
    assert_eq!(mids.get("BTC"), Some("80459.5".parse().unwrap()));
    assert_eq!(client.http().bodies()[0].0, "https://api.hyperliquid.xyz/info");
    assert_eq!(client.spent().minute, 2, "allMids weighs 2");
}

/// Claims one address and signs with another key, like a wallet that
/// switched accounts.
struct Impostor {
    key: LocalKey,
    claims: Address,
}

impl Signer for Impostor {
    fn address(&self) -> Address {
        self.claims
    }

    fn sign_typed(
        &self,
        domain: &Domain,
        data: &TypedStruct<'_>,
    ) -> impl Future<Output = Result<Signature, SignError>> {
        ready(self.key.sign_digest(&data.digest(domain)))
    }
}

#[test]
fn a_signature_from_anyone_but_the_signer_is_never_posted() {
    let claims = LocalKey::from_bytes(&[8; 32]).unwrap().address();
    let impostor = Impostor { key: LocalKey::from_bytes(&[7; 32]).unwrap(), claims };
    let client = Client::new(Wire::answering(OK), Network::Mainnet, clock, impostor);
    let action = parse(r#"{"type":"noop"}"#).to_action().unwrap();
    let error = block_on(client.send(&action)).unwrap_err();
    assert!(matches!(error, SendError::Sign(SignError::WrongSigner { .. })), "{error:?}");
    assert!(client.http().bodies().is_empty(), "nothing went out");
}

#[test]
fn nonces_never_repeat_within_a_millisecond() {
    let client = Client::new(Wire::answering(OK), Network::Mainnet, clock, LocalKey::from_bytes(&[7; 32]).unwrap());
    let first = client.next_nonce();
    let second = client.next_nonce();
    assert_eq!((first, second), (clock(), clock() + 1));
}
