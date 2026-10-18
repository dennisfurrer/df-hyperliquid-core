//! HyperEVM's JSON-RPC: each call answered by its method, or by the contract it goes to; a raw
//! transaction with no answer given is answered with its hash, as a node does.

use df_hyperliquid_core::utils::hex;
use df_hyperliquid_core::utils::sign::keccak256;
use serde_json::{Value, json};

use super::{Answer, Sent, Venue, ok};

/// One JSON-RPC answer: for `method`, and for a call to `to` when given.
pub(super) struct Rpc {
    method: String,
    to: Option<String>,
    answer: Value,
}

impl Venue {
    /// Answers every JSON-RPC call to `method` with `answer`, a whole JSON-RPC answer as a node
    /// writes it; its `id` is set to the request's.
    #[must_use]
    pub fn rpc(mut self, method: &str, answer: &str) -> Venue {
        self.rpc.push(Rpc { method: method.to_owned(), to: None, answer: serde_json::from_str(answer).unwrap() });
        self
    }

    /// Answers every `eth_call` to the contract at `to` with `answer`.
    #[must_use]
    pub fn rpc_call(self, to: &str, answer: &str) -> Venue {
        self.rpc_to("eth_call", to, answer)
    }

    /// Answers every call to `method` whose call object goes to `to` with `answer`: an
    /// `eth_estimateGas` for one contract, say. The first match wins, so give these before a
    /// [`Venue::rpc`] for the same method.
    #[must_use]
    pub fn rpc_to(mut self, method: &str, to: &str, answer: &str) -> Venue {
        self.rpc.push(Rpc {
            method: method.to_owned(),
            to: Some(to.to_ascii_lowercase()),
            answer: serde_json::from_str(answer).unwrap(),
        });
        self
    }

    pub(super) fn call(&self, body: &str, failure: Option<Answer>) -> Answer {
        let call: Value = serde_json::from_str(body)
            .unwrap_or_else(|error| panic!("a JSON-RPC body that is not JSON ({error}): {body}"));
        self.sent.borrow_mut().push(Sent::Rpc(call.clone()));
        failure.unwrap_or_else(|| ok(self.node(&call, body)))
    }

    fn node(&self, call: &Value, body: &str) -> String {
        let method = call["method"].as_str().unwrap_or_else(|| panic!("a JSON-RPC call with no method: {body}"));
        let to = call["params"][0]["to"].as_str().map(str::to_ascii_lowercase);
        let route = self.rpc.iter().find(|rpc| rpc.method == method && (rpc.to.is_none() || rpc.to == to));
        let mut answer = match (route, method) {
            (Some(rpc), _) => rpc.answer.clone(),
            (None, "eth_sendRawTransaction") => {
                let raw = hex::decode(call["params"][0].as_str().unwrap_or_default()).unwrap();
                json!({"jsonrpc": "2.0", "result": hex::encode_prefixed(&keccak256(&raw))})
            }
            (None, _) => panic!("the node has no answer for {body}"),
        };
        answer["id"] = call["id"].clone();
        answer.to_string()
    }
}
