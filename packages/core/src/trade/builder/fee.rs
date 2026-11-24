//! A builder's fee in tenths of a basis point, as an order's `f` and `maxBuilderFee` count it (10
//! is 1 basis point). `approveBuilderFee` names the same figure as a percent.

use crate::utils::dec::{Dec, Round};
use crate::utils::sign::Map;
use crate::utils::{Address, AssetKind};
use serde::Deserialize;
use std::fmt;

/// Tenths of a basis point: 10 is 0.01%, 1000 is 1%.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize)]
#[serde(transparent)]
pub struct BuilderFee(u64);

/// The most a builder may take on a perp order, main dex or HIP-3: 0.1%.
pub const PERP_BUILDER_CAP: BuilderFee = BuilderFee(100);

/// The most a builder may take on a spot order: 1%.
pub const SPOT_BUILDER_CAP: BuilderFee = BuilderFee(1_000);

/// HIP-4 takes builder codes as spot does.
pub const OUTCOME_BUILDER_CAP: BuilderFee = SPOT_BUILDER_CAP;

impl BuilderFee {
    /// No fee.
    pub const ZERO: BuilderFee = BuilderFee(0);

    /// A fee of `tenths` tenths of a basis point.
    #[must_use]
    pub const fn from_tenths(tenths: u64) -> BuilderFee {
        BuilderFee(tenths)
    }

    /// The fee in tenths of a basis point, as an order's `f` writes it.
    #[must_use]
    pub const fn tenths(self) -> u64 {
        self.0
    }

    /// Whether the venue lets a builder take this fee on an order of `kind`: `Err` with the cap
    /// when it is over it.
    pub const fn allowed_on(self, kind: AssetKind) -> Result<(), OverCap> {
        let cap = kind.builder_cap();
        match self.0 > cap.0 {
            true => Err(OverCap { fee: self, cap, kind }),
            false => Ok(()),
        }
    }

    /// The percent string `approveBuilderFee` takes as `maxFeeRate`: 880 is `0.88%`, 100 is `0.1%`,
    /// 1000 is `1%`. It always matches `^[0-9]+(\.[0-9]+)?%$`.
    #[must_use]
    pub fn percent(self) -> String {
        let figure = Dec::lit::<3>(i128::from(self.0)).to_wire();
        format!("{figure}%")
    }

    /// A percent string back in tenths. `None` for anything that is not a
    /// whole number of tenths of a basis point.
    #[must_use]
    pub fn from_percent(text: &str) -> Option<BuilderFee> {
        let digits = text.trim().strip_suffix('%')?;
        if !digits.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
            return None;
        }
        let figure = Dec::parse(digits).ok()?;
        if figure.places() > 3 {
            return None;
        }
        u64::try_from(figure.units_at(3, Round::Down)?).ok().map(BuilderFee)
    }
}

impl fmt::Display for BuilderFee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.percent())
    }
}

/// Who routed the order and what it takes: `{b, f}`, the address lowercase,
/// the fee in tenths of a basis point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builder {
    address: Address,
    fee: BuilderFee,
}

impl Builder {
    /// The account must have approved at least `fee` for this builder
    /// (`approveBuilderFee`), or the venue refuses the order.
    #[must_use]
    pub const fn new(address: Address, fee: BuilderFee) -> Builder {
        Builder { address, fee }
    }

    /// `b`: the builder's address.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    /// `f`: what the builder takes, in tenths of a basis point.
    #[must_use]
    pub const fn fee(&self) -> BuilderFee {
        self.fee
    }

    /// `{b, f}`, as an `order` action carries it.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("b", self.address.to_hex()).put("f", self.fee.tenths())
    }
}

/// A builder fee over what the venue lets a builder take on these orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the builder fee of {} is over the {} {} order allows ({})", .fee.tenths(), .cap.tenths(), .kind.one(), .cap)]
#[non_exhaustive]
pub struct OverCap {
    /// The fee asked for.
    pub fee: BuilderFee,
    /// The most the market kind allows.
    pub cap: BuilderFee,
    /// The market kind whose cap applies: the strictest among the orders.
    pub kind: AssetKind,
}

#[cfg(test)]
#[path = "tests/fee.rs"]
mod tests;
