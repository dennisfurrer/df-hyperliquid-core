//! A perp dex's name, which is also the form of a spot deployer's label: both share one set of names.

use std::{fmt, str::FromStr};

use crate::deploy::error::BadDeploy;

/// A HIP-3 perp dex name or a spot deployer label: 2 to 4 lower case
/// letters. The two share one set of names, so a label cannot repeat a dex
/// and a dex cannot repeat a label. Whether a name is taken is for the
/// caller to ask `perpDexs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DexName(String);

impl DexName {
    /// The name as sent.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for DexName {
    type Err = BadDeploy;

    /// Refuses rather than lowercases: `XYZ` is not taken to mean `xyz`.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let letters = name.bytes().all(|byte| byte.is_ascii_lowercase());
        if !(2..=4).contains(&name.len()) || !letters {
            return Err(BadDeploy::Name(name.to_owned()));
        }
        Ok(DexName(name.to_owned()))
    }
}

impl fmt::Display for DexName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
