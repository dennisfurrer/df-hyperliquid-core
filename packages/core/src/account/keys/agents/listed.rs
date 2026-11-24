//! `extraAgents`: an account's named agents, each with the expiry its name set.

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::Address;
use serde::Deserialize;

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

/// One `extraAgents` row: a named agent and when it lapses.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Agent {
    /// The agent's name; empty when the venue leaves it out.
    #[serde(default)]
    pub name: String,
    /// The agent's address.
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

#[cfg(test)]
#[path = "tests/listed.rs"]
mod tests;
