//! A fake Hyperliquid behind the [`Http`] wire, for whole journeys through a [`Client`]:
//!
//! - `/info` answers each read from the recording it was given for that `type` ([`info`]).
//! - `/exchange` checks every action the way the venue does before it answers: the nonce is
//!   fresh and inside the venue's window, and an L1 action's signer is recovered from the JSON
//!   exactly as posted (re-packed as msgpack, key order kept), so a body that differs from what
//!   was signed shows up as the wrong signer. Answers come from a queue ([`exchange`]).
//! - HyperEVM's JSON-RPC answers by method, or by the contract a call goes to; a raw transaction
//!   with its hash, as a node does ([`rpc`]).
//! - A failure can stand in for the next answer on a path: an HTTP status, or no answer at all.
//!   The request is still taken, so a test sees what reached the venue ([`failure`]).
//!
//! Anything it was not given an answer for fails the test, naming the request.
//!
//! [`Client`]: df_hyperliquid_core::Client

mod exchange;
mod failure;
mod info;
mod rpc;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::{Future, ready};

use df_hyperliquid_core::client::{Http, Method, Network, Request, Response, TransportError};
use df_hyperliquid_core::utils::{Address, Json};
use serde_json::Value;

pub use exchange::Signed;

pub struct Venue {
    network: Network,
    reads: Vec<info::Read>,
    answers: RefCell<VecDeque<String>>,
    rpc: Vec<rpc::Rpc>,
    signer: Option<Address>,
    failures: RefCell<VecDeque<failure::Failure>>,
    sent: RefCell<Vec<Sent>>,
}

/// What the wire hands back.
type Answer = Result<Response, TransportError>;

/// A request the venue took.
#[derive(Debug, Clone)]
pub enum Sent {
    Info(Json),
    Exchange(Signed),
    Rpc(Value),
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
            failures: RefCell::default(),
            sent: RefCell::default(),
        }
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

    fn answer(&self, request: &Request) -> Answer {
        assert_eq!(request.method, Method::Post, "the venue only takes posts: {}", request.url);
        let body = String::from_utf8(request.body.clone()).unwrap();
        let api = self.network.api_url();
        let failure = self.failure(&request.url);
        if request.url == format!("{api}/info") {
            self.read(&body, failure)
        } else if request.url == format!("{api}/exchange") {
            self.act(&body, failure)
        } else if request.url == self.network.evm_rpc_url() {
            self.call(&body, failure)
        } else {
            panic!("the venue does not serve {}", request.url)
        }
    }
}

impl Http for Venue {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        ready(self.answer(&request))
    }
}

fn ok(text: String) -> Answer {
    Ok(Response { status: 200, headers: Vec::new(), body: text.into_bytes() })
}
