//! Whether an address may be approved as an agent: one the venue has never seen, never listed and
//! never retired, since what it signed before could be replayed.

use crate::account::keys::agents::listed::Agent;
use crate::account::keys::agents::user_role::Role;
use crate::utils::Address;

/// Why an address may not be approved as an agent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Reused {
    /// The address is already among the account's agents (`extraAgents`).
    #[error("this address is one of the account's agents already")]
    Listed,
    /// The caller retired the address as an agent before.
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

#[cfg(test)]
#[path = "tests/fresh.rs"]
mod tests;
