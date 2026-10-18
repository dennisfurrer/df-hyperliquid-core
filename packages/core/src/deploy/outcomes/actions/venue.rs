//! A venue's name: the one an outcome deployer claims for good when it activates.

use std::{fmt, str::FromStr};

use crate::deploy::outcomes::actions::error::DeployError;

/// A venue name: 2 to 4 lower case letters, never `spot`. Activating claims
/// it for good; it stays taken after deactivating. Whether a perp dex holds
/// the name already is for the caller to ask `perpDexs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Venue(String);

impl Venue {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Venue {
    type Err = DeployError;

    /// Refuses rather than lowercases: `AB` is not taken to mean `ab`.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let letters = name.bytes().all(|byte| byte.is_ascii_lowercase());
        if !(2..=4).contains(&name.len()) || !letters {
            return Err(DeployError::Venue(name.to_owned()));
        }
        if name == "spot" {
            return Err(DeployError::SpotVenue);
        }
        Ok(Venue(name.to_owned()))
    }
}

impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "tests/venue.rs"]
mod tests;
