//! A named agent's name, and the `valid_until` form the venue reads an expiry out of.

use std::fmt;

/// The longest agent name, in characters.
pub const LONGEST_AGENT_NAME: usize = 16;

/// What sets an expiry inside a name.
pub(super) const VALID_UNTIL: &str = " valid_until ";

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

/// `<name> valid_until <ms>` back into the name and the expiry. A name
/// without the form is itself, with none.
#[must_use]
pub fn split_valid_until(name: &str) -> (&str, Option<u64>) {
    match name.rsplit_once(VALID_UNTIL) {
        Some((base, ms)) if !ms.is_empty() && ms.bytes().all(|b| b.is_ascii_digit()) => (base, ms.parse().ok()),
        _ => (name, None),
    }
}

#[cfg(test)]
#[path = "tests/name.rs"]
mod tests;
