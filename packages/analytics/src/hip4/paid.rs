//! What outcome fills paid, and what of it the venue and a deployer kept. A fill's fee is
//! everything it was billed, its builder's fee included (hl-hip4's [`Fill`]); the venue keeps the
//! rest, and the fee model (D11) gives the deployer `scale / (scale + max(scale, 1))` of that.
//! Only the closing side is billed, so a rate is measured on the fills that paid.

use std::collections::BTreeMap;

use hl_core::{Dec, Round};
use hl_hip4::{Catalog, Fill, multiplier};

use crate::{
    estimate::{Basis, Estimate},
    math::add,
};

/// Hundredths of a basis point in the whole: a rate of 1344 is 13.44 bp.
pub const PER_RATE: u128 = 1_000_000;

/// A set of fills summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Paid {
    pub fills: usize,
    /// Fills that took liquidity.
    pub taken: usize,
    pub notional: Dec,
    /// Notional of the fills that were billed: on an outcome, the closing side.
    pub charged: Dec,
    /// Everything billed, builders' fees included.
    pub fee: Dec,
    pub builder: Dec,
    /// Realised profit, as the venue reports it, before the fee.
    pub pnl: Dec,
}

impl Paid {
    #[must_use]
    pub fn of<'a>(fills: impl IntoIterator<Item = &'a Fill>) -> Paid {
        fills.into_iter().fold(Paid::default(), Paid::with)
    }

    /// This and one fill more.
    #[must_use]
    pub fn with(self, fill: &Fill) -> Paid {
        let worth = fill.notional().abs();
        Paid {
            fills: self.fills + 1,
            taken: self.taken + usize::from(fill.crossed),
            notional: add(self.notional, worth),
            charged: if fill.fee.is_zero() { self.charged } else { add(self.charged, worth) },
            fee: add(self.fee, fill.fee),
            builder: add(self.builder, fill.builder_fee.unwrap_or(Dec::ZERO)),
            pnl: add(self.pnl, fill.closed_pnl),
        }
    }

    /// Two sets as one.
    #[must_use]
    pub fn plus(self, other: Paid) -> Paid {
        Paid {
            fills: self.fills + other.fills,
            taken: self.taken + other.taken,
            notional: add(self.notional, other.notional),
            charged: add(self.charged, other.charged),
            fee: add(self.fee, other.fee),
            builder: add(self.builder, other.builder),
            pnl: add(self.pnl, other.pnl),
        }
    }

    /// The fee over what was billed, in hundredths of a basis point, cut down; `None` for
    /// nothing billed.
    #[must_use]
    pub fn rate(&self) -> Option<u32> {
        rate_of(self.fee, self.charged)
    }

    /// The fee over everything traded, in hundredths of a basis point.
    #[must_use]
    pub fn blended(&self) -> Option<u32> {
        rate_of(self.fee, self.notional)
    }

    /// What the venue kept: the fee less the builders'.
    #[must_use]
    pub fn venue(&self) -> Dec {
        self.fee.checked_sub(self.builder).unwrap_or(self.fee)
    }

    /// The deployer's part of what the venue kept, on markets of fee scale `scale`, to eight
    /// places toward zero.
    #[must_use]
    pub fn deployer(&self, scale: Dec) -> Option<Estimate> {
        let amount = self.venue().checked_mul(scale)?.checked_div(multiplier(scale)?, 8, Round::Down)?;
        Some(Estimate { amount, basis: Basis::Fills { fills: self.fills } })
    }

    /// Whether every fill took.
    #[must_use]
    pub const fn all_taken(&self) -> bool {
        self.fills == self.taken
    }
}

/// `fee` over `notional` in hundredths of a basis point, cut down; `None` for no fee, nothing
/// traded, or a rate below one.
fn rate_of(fee: Dec, notional: Dec) -> Option<u32> {
    if !fee.is_positive() || !notional.is_positive() {
        return None;
    }
    let units = fee.checked_div(notional, 6, Round::Down)?.units_at(6, Round::Down)?;
    match units {
        0 => None,
        units => Some(u32::try_from(units).unwrap_or(u32::MAX)),
    }
}

/// One venue's part of a set of fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Take<'a> {
    /// `None` for the venue's own daily markets.
    pub venue: Option<&'a str>,
    pub paid: Paid,
    /// What its deployer was paid, over the fills whose market's scale is known; `None` for the
    /// venue's own markets, which pay no deployer.
    pub deployer: Option<Estimate>,
    /// Fills left out of the estimate: their market lists no fee scale.
    pub unscaled: usize,
}

/// A set of fills split by venue.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Takes<'a> {
    /// The venues, the most traded first.
    pub venues: Vec<Take<'a>>,
    /// Fills on a market the catalogue no longer lists.
    pub unlisted: Paid,
}

/// One venue's fills: all of them, those whose scale is known by scale, and how many have none.
type Split = (Paid, BTreeMap<Dec, Paid>, usize);

/// `fills` split by the venue that listed each one's market, each at its market's own fee
/// scale, or at `assumed` where the market lists none.
#[must_use]
pub fn by_venue<'a>(fills: &[Fill], catalog: &'a Catalog, assumed: Option<Dec>) -> Takes<'a> {
    // Each venue's fills, kept apart by scale so each group is estimated at its own.
    let mut split: BTreeMap<Option<&'a str>, Split> = BTreeMap::new();
    let mut unlisted = Paid::default();
    for fill in fills {
        let Some(market) = catalog.find(fill.coin) else {
            unlisted = unlisted.with(fill);
            continue;
        };
        let (paid, scaled, unscaled) = split.entry(market.venue.as_deref()).or_default();
        *paid = paid.with(fill);
        match market.fee_scale.or(assumed) {
            Some(scale) => {
                let group = scaled.entry(scale).or_default();
                *group = group.with(fill);
            }
            None => *unscaled += 1,
        }
    }
    let mut venues: Vec<Take<'a>> = split
        .into_iter()
        .map(|(venue, (paid, scaled, unscaled))| {
            let deployer = venue.map(|_| {
                let fills = scaled.values().map(|group| group.fills).sum();
                let amount = scaled
                    .iter()
                    .filter_map(|(scale, group)| group.deployer(*scale))
                    .fold(Dec::ZERO, |sum, estimate| add(sum, estimate.amount));
                Estimate { amount, basis: Basis::Fills { fills } }
            });
            Take { venue, paid, deployer, unscaled }
        })
        .collect();
    venues.sort_by(|a, b| b.paid.notional.cmp(&a.paid.notional).then_with(|| a.venue.cmp(&b.venue)));
    Takes { venues, unlisted }
}

#[cfg(test)]
#[path = "../tests/paid.rs"]
mod tests;
