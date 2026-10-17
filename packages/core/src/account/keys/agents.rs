//! API wallets (agents): `approveAgent`, an account's named agents (`extraAgents`) and what the
//! venue knows an address as (`userRole`). An agent signs L1 actions for its master and never
//! moves funds; there is no revoke, so an address that was an agent is never approved again.

use std::fmt;

use crate::account::Signature;
use crate::client::{ApiError, Network, Transport, body, info, user_request};
use crate::utils::Address;
use crate::utils::sign::{Field, Map, NonceKey, UserAction, UserSigned, Value};
use serde::Deserialize;

/// The longest an approval may run: 180 days ahead.
pub const MAX_AGENT_VALIDITY_MS: u64 = 180 * 86_400_000;

/// The longest agent name, in characters.
pub const LONGEST_AGENT_NAME: usize = 16;

/// What sets an expiry inside a name.
const VALID_UNTIL: &str = " valid_until ";

/// A named agent's name: 1 to 16 characters, not blank, and never the
/// `valid_until` form, which the venue reads as an expiry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentName(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("an agent's name is 1 to {LONGEST_AGENT_NAME} characters and not a valid_until form, which `{0}` is not")]
pub struct BadAgentName(pub String);

impl AgentName {
    pub fn new(name: &str) -> Result<AgentName, BadAgentName> {
        let fits = !name.trim().is_empty() && name.chars().count() <= LONGEST_AGENT_NAME;
        match fits && !name.contains("valid_until") {
            true => Ok(AgentName(name.to_owned())),
            false => Err(BadAgentName(name.to_owned())),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An expiry that does not lie ahead of `now`, or lies more than 180 days ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an agent's approval ends ahead of now ({now}) and within 180 days, which {until} does not")]
pub struct BadExpiry {
    pub until: u64,
    pub now: u64,
}

/// `<name> valid_until <ms>` back into the name and the expiry. A name
/// without the form is itself, with none.
#[must_use]
pub fn split_valid_until(name: &str) -> (&str, Option<u64>) {
    match name.rsplit_once(VALID_UNTIL) {
        Some((base, ms)) if !ms.is_empty() && ms.bytes().all(|b| b.is_ascii_digit()) => (base, ms.parse().ok()),
        _ => (name, None),
    }
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

/// Why an address may not be approved as an agent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Reused {
    #[error("this address is one of the account's agents already")]
    Listed,
    #[error("this address was an agent before, and what it signed then could be replayed")]
    Retired,
    /// The venue knows the address already, as this.
    #[error("the venue knows this address already, as {0:?}; an agent is a fresh address")]
    Known(Role),
}

/// Whether `candidate` may be approved as an agent: an address the venue has
/// never seen (`userRole` answers `missing`), not among the account's agents
/// (`extraAgents`) and not one it retired, which only the caller remembers.
pub fn fresh_agent(candidate: Address, role: &Role, listed: &[Agent], retired: &[Address]) -> Result<(), Reused> {
    if listed.iter().any(|agent| agent.address == candidate) {
        return Err(Reused::Listed);
    }
    if retired.contains(&candidate) {
        return Err(Reused::Retired);
    }
    match role {
        Role::Missing => Ok(()),
        known => Err(Reused::Known(known.clone())),
    }
}

/// `{"type":"extraAgents","user":..}`.
#[must_use]
pub fn extra_agents_request(user: Address) -> String {
    user_request("extraAgents", user)
}

/// The account's named agents. The venue has answered both `null` and `[]`
/// for an account with none; both read as none.
pub async fn extra_agents(transport: &impl Transport, user: Address) -> Result<Vec<Agent>, ApiError> {
    let agents: Option<Vec<Agent>> = info(transport, extra_agents_request(user)).await?;
    Ok(agents.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    #[serde(default)]
    pub name: String,
    pub address: Address,
    /// Milliseconds; from it on the agent signs nothing.
    #[serde(default)]
    pub valid_until: Option<u64>,
}

impl Agent {
    /// Whether it still signs at `now` milliseconds.
    #[must_use]
    pub fn is_live(&self, now: u64) -> bool {
        self.valid_until.is_none_or(|until| until > now)
    }
}

/// `{"type":"userRole","user":..}`.
#[must_use]
pub fn role_request(user: Address) -> String {
    user_request("userRole", user)
}

pub async fn role(transport: &impl Transport, user: Address) -> Result<Role, ApiError> {
    info(transport, role_request(user)).await
}

/// What the venue knows an address as.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawRole")]
pub enum Role {
    User,
    /// An address the venue has never seen.
    Missing,
    Vault,
    /// An agent, and the account it signs for.
    Agent {
        user: Address,
    },
    /// A subaccount, and its master.
    SubAccount {
        master: Address,
    },
    /// A role this build does not know, or a known one whose data did not
    /// read, kept as spelled.
    Other(String),
}

impl Role {
    /// The account an agent signs for.
    #[must_use]
    pub const fn agent_of(&self) -> Option<Address> {
        match self {
            Role::Agent { user } => Some(*user),
            _ => None,
        }
    }

    /// A subaccount's master.
    #[must_use]
    pub const fn master(&self) -> Option<Address> {
        match self {
            Role::SubAccount { master } => Some(*master),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct RawRole {
    #[serde(default)]
    role: String,
    #[serde(default)]
    data: Option<RawRoleData>,
}

#[derive(Deserialize)]
struct RawRoleData {
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    master: Option<String>,
}

impl From<RawRole> for Role {
    fn from(raw: RawRole) -> Role {
        let named = |pick: fn(&RawRoleData) -> Option<&String>| -> Option<Address> {
            raw.data.as_ref().and_then(pick).and_then(|text| text.parse().ok())
        };
        match raw.role.as_str() {
            "user" => Role::User,
            "missing" => Role::Missing,
            "vault" => Role::Vault,
            "agent" => {
                named(|data| data.user.as_ref()).map_or(Role::Other(raw.role.clone()), |user| Role::Agent { user })
            }
            "subAccount" => named(|data| data.master.as_ref())
                .map_or(Role::Other(raw.role.clone()), |master| Role::SubAccount { master }),
            _ => Role::Other(raw.role.clone()),
        }
    }
}

#[cfg(test)]
#[path = "tests/agents.rs"]
mod tests;
