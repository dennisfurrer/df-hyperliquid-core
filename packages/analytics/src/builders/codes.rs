//! How a builder fee works, as numbers. A fee is counted in tenths of a basis point (10 is one
//! basis point, 1,000 one percent) and capped by the kind of market: a tenth of a percent on
//! perps, native and HIP-3, and one percent on spot and on HIP-4 outcomes, which take builder
//! codes as spot does. A fill names one builder or none, and pays it on the fill's whole
//! notional, buys included; the trader approves the most a builder may take once, with
//! `approveBuilderFee`. Splits, merges and settlements name nobody.

use hl_core::{AssetKind, BuilderFee, Dec, Round};

/// Tenths of a basis point in one basis point.
pub const TENTHS_PER_BASIS_POINT: u64 = 10;

/// hip4-tui's D15: every order carries its builder at 88% of its market's cap, 88 tenths of a
/// basis point on a perp and 880 on spot and outcomes.
pub const SHARE_OF_CAP: u64 = 88;

/// One market kind's cap, and the fee a share of it comes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cap {
    pub kind: AssetKind,
    pub cap: BuilderFee,
    /// `share` percent of the cap, cut down to a whole tenth of a basis point.
    pub fee: BuilderFee,
    /// Percent of the cap.
    pub share: u64,
}

/// Every market kind's cap and what `percent` of it charges, in hl-core's order of kinds. `None`
/// over 100: no share takes more than the cap.
#[must_use]
pub fn schedule(percent: u64) -> Option<[Cap; 4]> {
    (percent <= 100).then(|| {
        AssetKind::ALL.map(|kind| {
            let cap = kind.builder_cap();
            Cap { kind, cap, fee: BuilderFee::from_tenths(cap.tenths().saturating_mul(percent) / 100), share: percent }
        })
    })
}

/// A fee as a fraction of notional: `0.0001` for 10.
#[must_use]
pub fn rate(fee: BuilderFee) -> Dec {
    Dec::new(i128::from(fee.tenths()), 5).unwrap_or(Dec::ZERO)
}

/// What a fill of `notional` pays a builder charging `fee`.
#[must_use]
pub fn paid(fee: BuilderFee, notional: Dec) -> Option<Dec> {
    notional.checked_mul(rate(fee))
}

/// How much of `kind`'s cap `fee` takes, in hundredths of a percent; `None` over the cap.
#[must_use]
pub fn share_of(fee: BuilderFee, kind: AssetKind) -> Option<u32> {
    let cap = kind.builder_cap();
    if fee > cap || cap == BuilderFee::ZERO {
        return None;
    }
    let part = Dec::from_int(i64::try_from(fee.tenths()).ok()?);
    let whole = Dec::from_int(i64::try_from(cap.tenths()).ok()?);
    u32::try_from(part.checked_div(whole, 4, Round::Down)?.units_at(4, Round::Down)?).ok()
}

#[cfg(test)]
#[path = "../tests/codes.rs"]
mod tests;
