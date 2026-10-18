//! `/exchange`: every action checked as the venue checks it before an answer comes off the
//! queue. The nonce must be fresh and inside the venue's window; an L1 action's signer is
//! recovered from the JSON as posted; a user-signed action must name this network's chain.

use df_hyperliquid_core::account::{Signature, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::utils::sign::l1_digest;
use df_hyperliquid_core::utils::{Address, Json, hex};

use super::{Answer, Sent, Venue, ok};
use crate::support::{NOW, fixture};

/// The venue takes nonces from two days before its clock to one day after.
const NONCE_BEHIND_MS: u64 = 2 * 24 * 60 * 60 * 1_000;
const NONCE_AHEAD_MS: u64 = 24 * 60 * 60 * 1_000;

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

    pub(super) fn act(&self, body: &str, failure: Option<Answer>) -> Answer {
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
        failure.unwrap_or_else(|| {
            let answer = self.answers.borrow_mut().pop_front();
            ok(answer.unwrap_or_else(|| panic!("the venue has no answer queued for {body}")))
        })
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
