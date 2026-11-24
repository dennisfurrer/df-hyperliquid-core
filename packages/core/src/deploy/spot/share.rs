//! The deployer trading fee share: the part of a token's trading fees its deployer keeps, sent as a
//! percentage string.

use std::{fmt, str::FromStr};

use crate::deploy::error::BadDeploy;
use crate::utils::Dec;

/// What a token's deployer keeps of its trading fees, from `0%` to `100%`.
/// It starts at 100% and can be sent again only to lower it. Sent as the
/// percentage with no trailing zeros: `99.40%` as `99.4%`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeeShare(Dec);

impl FeeShare {
    /// 100%, the share a token starts with.
    pub const ALL: FeeShare = FeeShare(Dec::lit::<0>(100));

    /// `percent` per cent, from 0 to 100.
    pub fn percent(percent: Dec) -> Result<Self, BadDeploy> {
        if percent.is_negative() || percent > Dec::from_int(100) {
            return Err(BadDeploy::FeeShare(format!("{percent}%")));
        }
        Ok(FeeShare(percent))
    }

    /// The share in per cent.
    #[must_use]
    pub const fn value(self) -> Dec {
        self.0
    }

    /// The string hashed, such as `0.012%`.
    #[must_use]
    pub fn to_wire(self) -> String {
        format!("{}%", self.0.to_wire())
    }
}

impl FromStr for FeeShare {
    type Err = BadDeploy;

    /// Takes `99.4%` or `100%`; the sign is required, as the venue writes it.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || BadDeploy::FeeShare(text.trim().to_owned());
        let number = text.trim().strip_suffix('%').ok_or_else(bad)?;
        if number.starts_with(['+', '-']) || number.trim() != number {
            return Err(bad());
        }
        let percent = Dec::parse(number).map_err(|_| bad())?;
        FeeShare::percent(percent).map_err(|_| bad())
    }
}

impl fmt::Display for FeeShare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}
