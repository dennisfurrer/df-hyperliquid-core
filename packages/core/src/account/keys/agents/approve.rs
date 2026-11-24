//! `approveAgent`: an agent approved under a name, until an expiry, or unnamed, signed by the
//! account's own wallet.

use crate::account::Signature;
use crate::account::keys::agents::name::{AgentName, VALID_UNTIL};
use crate::client::Network;
use crate::client::http::body;
use crate::utils::Address;
use crate::utils::sign::{Field, Map, NonceKey, UserAction, UserSigned, Value};

/// The longest an approval may run: 180 days ahead.
pub const MAX_AGENT_VALIDITY_MS: u64 = 180 * 86_400_000;

/// An expiry that does not lie ahead of `now`, or lies more than 180 days ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an agent's approval ends ahead of now ({now}) and within 180 days, which {until} does not")]
pub struct BadExpiry {
    /// The expiry asked for, in milliseconds.
    pub until: u64,
    /// The time it was checked against, in milliseconds.
    pub now: u64,
}

/// `approveAgent`, signed by the account's own wallet. There is no revoke: approving a name again
/// replaces that agent. Never approve an address that was an agent before, since what it signed
/// then can be replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveAgent {
    agent: Address,
    name: String,
    /// The unnamed agent, posted with `agentName` null.
    null: bool,
    nonce: u64,
}

impl ApproveAgent {
    /// Under `name` as given. Empty approves the unnamed agent, signed and posted as the empty string.
    #[must_use]
    pub fn new(agent: Address, name: &str, nonce: u64) -> ApproveAgent {
        ApproveAgent { agent, name: name.to_owned(), null: false, nonce }
    }

    /// The unnamed agent, posted with `agentName` null and signed as the empty string: the digest is
    /// the empty name's, only the posted JSON differs.
    #[must_use]
    pub const fn unnamed(agent: Address, nonce: u64) -> ApproveAgent {
        ApproveAgent { agent, name: String::new(), null: true, nonce }
    }

    /// A named agent that never lapses.
    #[must_use]
    pub fn named(agent: Address, name: &AgentName, nonce: u64) -> ApproveAgent {
        ApproveAgent::new(agent, name.as_str(), nonce)
    }

    /// A named agent until `until` milliseconds: `<name> valid_until <until>`,
    /// refused unless it lies ahead of `now` and at most 180 days ahead.
    pub fn expiring(
        agent: Address,
        name: &AgentName,
        until: u64,
        now: u64,
        nonce: u64,
    ) -> Result<ApproveAgent, BadExpiry> {
        let ahead = until.checked_sub(now).filter(|ahead| *ahead > 0 && *ahead <= MAX_AGENT_VALIDITY_MS);
        match ahead {
            Some(_) => Ok(ApproveAgent::new(agent, &format!("{name}{VALID_UNTIL}{until}"), nonce)),
            None => Err(BadExpiry { until, now }),
        }
    }

    /// The agent's address.
    #[must_use]
    pub const fn agent(&self) -> Address {
        self.agent
    }

    /// `agentName` as signed; `None` for the unnamed agent.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        Some(self.name.as_str()).filter(|name| !name.is_empty())
    }
}

impl UserAction for ApproveAgent {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("approveAgent", "HyperliquidTransaction:ApproveAgent", NonceKey::Nonce, self.nonce)
            .field("agentAddress", Field::Address(self.agent))
            .field("agentName", Field::String(&self.name))
    }

    fn action(&self, network: Network) -> Map {
        let action = self.user_signed().action(network);
        if !self.null {
            return action;
        }
        action.into_pairs().fold(Map::new(), |map, (key, value)| match key == "agentName" {
            true => map.put(key, Value::Nil),
            false => map.put(key, value),
        })
    }

    fn wire(&self, network: Network) -> String {
        self.action(network).to_json().compact()
    }

    fn body(&self, signature: &Signature, network: Network) -> String {
        body::user_signed(&self.action(network), self.nonce, signature)
    }
}

#[cfg(test)]
#[path = "tests/approve.rs"]
mod tests;
