//! A fake Hyperliquid behind the [`Http`] wire, for whole journeys through a [`Client`]:
//!
//! - `/info` answers each read from the recording it was given for that `type`.
//! - `/exchange` checks every action the way the venue does before it answers: the nonce is
//!   fresh and inside the venue's window, and an L1 action's signer is recovered from the JSON
//!   exactly as posted (re-packed as msgpack, key order kept), so a body that differs from what
//!   was signed shows up as the wrong signer. Answers come from a queue.
//! - HyperEVM's JSON-RPC answers by method, or by the contract an `eth_call` goes to.
//!
//! Anything it was not given an answer for fails the test, naming the request.
//!
//! [`Client`]: df_hyperliquid_core::Client

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::{Future, ready};

use df_hyperliquid_core::account::{Signature, recover};
use df_hyperliquid_core::client::{Http, Method, Network, Request, Response, TransportError};
use df_hyperliquid_core::utils::sign::l1_digest;
use df_hyperliquid_core::utils::{Address, Json, hex};
use serde_json::Value;

use super::{NOW, fixture};

/// The venue takes nonces from two days before its clock to one day after.
const NONCE_BEHIND_MS: u64 = 2 * 24 * 60 * 60 * 1_000;
const NONCE_AHEAD_MS: u64 = 24 * 60 * 60 * 1_000;

pub struct Venue {
    network: Network,
    reads: Vec<Read>,
    answers: RefCell<VecDeque<String>>,
    rpc: Vec<Rpc>,
    signer: Option<Address>,
    sent: RefCell<Vec<Sent>>,
}

/// One `/info` answer: for reads of `kind` whose fields match.
struct Read {
    kind: String,
    fields: Vec<(String, String)>,
    answer: String,
}

/// One JSON-RPC answer: for `method`, and for an `eth_call` to `to` when given.
struct Rpc {
    method: String,
    to: Option<String>,
    answer: Value,
}

/// A request the venue took.
#[derive(Debug, Clone)]
pub enum Sent {
    Info(Json),
    Exchange(Signed),
    Rpc(Value),
}

/// A signed action as the venue read it.
#[derive(Debug, Clone)]
pub struct Signed {
    pub action: Json,
    pub nonce: u64,
    pub vault: Option<Address>,
    /// Recovered from an L1 action's signature; `None` for a user-signed action, whose typed
    /// struct the venue would need per action type.
    pub signer: Option<Address>,
    /// `hyperliquidChain`, for a user-signed action.
    pub chain: Option<String>,
}

impl Signed {
    /// The action's `type`.
    pub fn kind(&self) -> &str {
        self.action.get("type").and_then(Json::as_str).unwrap_or_default()
    }
}

impl Venue {
    pub fn mainnet() -> Venue {
        Venue::on(Network::Mainnet)
    }

    pub fn on(network: Network) -> Venue {
        Venue {
            network,
            reads: Vec::new(),
            answers: RefCell::default(),
            rpc: Vec::new(),
            signer: None,
            sent: RefCell::default(),
        }
    }

    /// Answers every `{"type":kind}` read with the recording at `fixtures/<path>`.
    #[must_use]
    pub fn info(self, kind: &str, path: &str) -> Venue {
        self.info_text(kind, &fixture(path))
    }

    /// Answers every `{"type":kind}` read with `answer`.
    #[must_use]
    pub fn info_text(self, kind: &str, answer: &str) -> Venue {
        self.info_where(kind, &[], answer)
    }

    /// Answers `{"type":kind}` reads whose fields hold these values (strings as written, other
    /// values as compact JSON) with `answer`. The first match wins, so give narrow reads first.
    #[must_use]
    pub fn info_where(mut self, kind: &str, fields: &[(&str, &str)], answer: &str) -> Venue {
        let fields = fields.iter().map(|(key, value)| ((*key).to_owned(), (*value).to_owned())).collect();
        self.reads.push(Read { kind: kind.to_owned(), fields, answer: answer.to_owned() });
        self
    }

    /// Queues an answer from `fixtures/trade/exchange.json` by its name, for the next action.
    #[must_use]
    pub fn exchange(self, name: &str) -> Venue {
        let file = Json::parse(&fixture("trade/exchange.json")).unwrap();
        let answer = file
            .get("answers")
            .and_then(Json::as_array)
            .and_then(|answers| answers.iter().find(|answer| answer.get("name").and_then(Json::as_str) == Some(name)))
            .and_then(|answer| answer.get("answer"))
            .and_then(Json::as_str)
            .unwrap_or_else(|| panic!("trade/exchange.json has no answer {name}"))
            .to_owned();
        self.exchange_text(&answer)
    }

    /// Queues `answer` for the next action.
    #[must_use]
    pub fn exchange_text(self, answer: &str) -> Venue {
        self.answers.borrow_mut().push_back(answer.to_owned());
        self
    }

    /// Answers every JSON-RPC call to `method` with `answer`, a whole JSON-RPC answer as a node
    /// writes it; its `id` is set to the request's.
    #[must_use]
    pub fn rpc(mut self, method: &str, answer: &str) -> Venue {
        self.rpc.push(Rpc { method: method.to_owned(), to: None, answer: serde_json::from_str(answer).unwrap() });
        self
    }

    /// Answers every `eth_call` to the contract at `to` with `answer`.
    #[must_use]
    pub fn rpc_call(mut self, to: &str, answer: &str) -> Venue {
        self.rpc.push(Rpc {
            method: "eth_call".to_owned(),
            to: Some(to.to_ascii_lowercase()),
            answer: serde_json::from_str(answer).unwrap(),
        });
        self
    }

    /// Every L1 action must recover to `signer`.
    #[must_use]
    pub fn signed_by(mut self, signer: Address) -> Venue {
        self.signer = Some(signer);
        self
    }

    /// Every request, in order.
    pub fn sent(&self) -> Vec<Sent> {
        self.sent.borrow().clone()
    }

    /// The `/info` reads, in order.
    pub fn reads(&self) -> Vec<Json> {
        self.sent().into_iter().filter_map(|sent| if let Sent::Info(read) = sent { Some(read) } else { None }).collect()
    }

    /// The `type` of each `/info` read, in order.
    pub fn read_kinds(&self) -> Vec<String> {
        self.reads().iter().filter_map(|read| read.get("type").and_then(Json::as_str).map(ToOwned::to_owned)).collect()
    }

    /// The signed actions, in order.
    pub fn actions(&self) -> Vec<Signed> {
        self.sent()
            .into_iter()
            .filter_map(|sent| if let Sent::Exchange(signed) = sent { Some(signed) } else { None })
            .collect()
    }

    /// The JSON-RPC calls, in order.
    pub fn calls(&self) -> Vec<Value> {
        self.sent().into_iter().filter_map(|sent| if let Sent::Rpc(call) = sent { Some(call) } else { None }).collect()
    }

    fn answer(&self, request: &Request) -> Response {
        assert_eq!(request.method, Method::Post, "the venue only takes posts: {}", request.url);
        let body = String::from_utf8(request.body.clone()).unwrap();
        let api = self.network.api_url();
        if request.url == format!("{api}/info") {
            ok(self.read(&body))
        } else if request.url == format!("{api}/exchange") {
            ok(self.act(&body))
        } else if request.url == self.network.evm_rpc_url() {
            ok(self.call(&body))
        } else {
            panic!("the venue does not serve {}", request.url)
        }
    }

    fn read(&self, body: &str) -> String {
        let read = Json::parse(body).unwrap_or_else(|error| panic!("an /info body that is not JSON ({error}): {body}"));
        let kind =
            read.get("type").and_then(Json::as_str).unwrap_or_else(|| panic!("an /info read with no type: {body}"));
        let answer = self
            .reads
            .iter()
            .find(|route| route.kind == kind && route.fields.iter().all(|(key, value)| matches(read.get(key), value)))
            .unwrap_or_else(|| panic!("the venue has no answer for {body}"))
            .answer
            .clone();
        self.sent.borrow_mut().push(Sent::Info(read));
        answer
    }

    fn act(&self, body: &str) -> String {
        let posted =
            Json::parse(body).unwrap_or_else(|error| panic!("an /exchange body that is not JSON ({error}): {body}"));
        let action = posted.get("action").unwrap_or_else(|| panic!("no action in {body}")).clone();
        let nonce = posted.get("nonce").and_then(Json::as_u64).unwrap_or_else(|| panic!("no nonce in {body}"));
        let signature = signature(posted.get("signature").unwrap_or_else(|| panic!("no signature in {body}")));
        assert!(
            nonce > NOW - NONCE_BEHIND_MS && nonce < NOW + NONCE_AHEAD_MS,
            "nonce {nonce} is outside the venue's window around {NOW}"
        );
        assert!(self.actions().iter().all(|earlier| earlier.nonce != nonce), "nonce {nonce} was used before");
        let chain = action.get("hyperliquidChain").and_then(Json::as_str).map(ToOwned::to_owned);
        let (vault, signer) = if action.get("signatureChainId").is_some() {
            let expected = if self.network == Network::Mainnet { "Mainnet" } else { "Testnet" };
            assert_eq!(chain.as_deref(), Some(expected), "a user-signed action for another chain: {body}");
            (None, None)
        } else {
            let vault = match posted.get("vaultAddress") {
                Some(Json::String(vault)) => Some(vault.parse::<Address>().unwrap()),
                Some(Json::Null) => None,
                other => panic!("vaultAddress is {other:?}; the venue wants an address or null: {body}"),
            };
            let packed = action.to_action().unwrap_or_else(|error| panic!("{error}: {body}")).to_msgpack();
            let digest = l1_digest(&packed, nonce, vault, self.network);
            let signer = recover(&digest, &signature).unwrap_or_else(|error| panic!("{error}: {body}"));
            if let Some(expected) = self.signer {
                assert_eq!(signer, expected, "the action as posted recovers to another signer: {body}");
            }
            (vault, Some(signer))
        };
        self.sent.borrow_mut().push(Sent::Exchange(Signed { action, nonce, vault, signer, chain }));
        self.answers.borrow_mut().pop_front().unwrap_or_else(|| panic!("the venue has no answer queued for {body}"))
    }

    fn call(&self, body: &str) -> String {
        let call: Value = serde_json::from_str(body)
            .unwrap_or_else(|error| panic!("a JSON-RPC body that is not JSON ({error}): {body}"));
        let method = call["method"].as_str().unwrap_or_else(|| panic!("a JSON-RPC call with no method: {body}"));
        let to = call["params"][0]["to"].as_str().map(str::to_ascii_lowercase);
        let rpc = self
            .rpc
            .iter()
            .find(|rpc| rpc.method == method && (rpc.to.is_none() || rpc.to == to))
            .unwrap_or_else(|| panic!("the node has no answer for {body}"));
        let mut answer = rpc.answer.clone();
        answer["id"] = call["id"].clone();
        self.sent.borrow_mut().push(Sent::Rpc(call));
        answer.to_string()
    }
}

impl Http for Venue {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        ready(Ok(self.answer(&request)))
    }
}

fn ok(text: String) -> Response {
    Response { status: 200, headers: Vec::new(), body: text.into_bytes() }
}

fn matches(value: Option<&Json>, expected: &str) -> bool {
    match value {
        Some(Json::String(text)) => text.eq_ignore_ascii_case(expected),
        Some(other) => other.compact() == expected,
        None => false,
    }
}

/// `{"r":"0x..","s":"0x..","v":27}`, as `/exchange` reads it.
fn signature(json: &Json) -> Signature {
    let word = |key: &str| -> [u8; 32] {
        hex::decode_array(json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key} in the signature")))
            .unwrap()
    };
    let v = json.get("v").and_then(Json::as_u64).unwrap();
    Signature::from_parts(word("r"), word("s"), u8::try_from(v).unwrap())
}
