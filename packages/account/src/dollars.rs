use hl_core::Dec;

use crate::state::{Abstraction, PerpState, SpotState};

/// An account's USDC as one figure, per docs/V1.md "Account model". A
/// standard account keeps USDC in two places, spot and the perp balance; a
/// pooled one keeps it all in spot, and its perp figures are not counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dollars {
    /// Spot USDC `total`, which includes what is on hold.
    pub spot_total: Dec,
    /// Spot USDC `total - hold`.
    pub spot_available: Dec,
    /// Perp `withdrawable`; zero on a pooled account.
    pub perp_withdrawable: Dec,
    /// `spot_total + perp_withdrawable`.
    pub total: Dec,
    /// `spot_available + perp_withdrawable`.
    pub available: Dec,
}

impl Dollars {
    /// `None` only past what a `Dec` counts.
    #[must_use]
    pub fn new(abstraction: &Abstraction, spot: &SpotState, perp: &PerpState) -> Option<Dollars> {
        let (spot_total, spot_available) = match spot.usdc() {
            Some(usdc) => (usdc.total, usdc.available()?),
            None => (Dec::ZERO, Dec::ZERO),
        };
        let perp_withdrawable = match abstraction.is_pooled() {
            true => Dec::ZERO,
            false => perp.withdrawable,
        };
        Some(Dollars {
            spot_total,
            spot_available,
            perp_withdrawable,
            total: spot_total.checked_add(perp_withdrawable)?,
            available: spot_available.checked_add(perp_withdrawable)?,
        })
    }
}
