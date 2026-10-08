//! The reserve math: utilization, the borrow and supply rates, what a holding
//! is worth and backs, and what an account can still borrow. Exact, to the
//! ten places the venue writes rates in.
//!
//! The rules are the venue's portfolio-margin docs, checked against every
//! reserve on file (mainnet and testnet): utilization is borrowed over
//! supplied, and suppliers earn the borrow rate times utilization less the
//! tenth the protocol keeps. What can still be borrowed is atlas's
//! `borrowable`.

use hl_core::{Dec, Round, USDC_INDEX};

use crate::state::{ReserveState, Risk, UserState, reserve};

/// Rates and utilization are written to ten places.
pub const RATE_PLACES: u32 = 10;

/// The share of borrowers' interest the protocol keeps "as a buffer for
/// future liquidations": a tenth.
pub const PROTOCOL_SHARE: Dec = match Dec::new(1, 1) {
    Some(share) => share,
    None => Dec::ZERO,
};

/// The most USDC one account may borrow: the docs' user borrow cap.
pub const USER_BORROW_CAP: Dec = Dec::from_int(50_000_000);

/// The borrow rate's floor, 5% a year.
const BASE_RATE: Dec = match Dec::new(5, 2) {
    Some(rate) => rate,
    None => Dec::ZERO,
};
/// Where the rate turns up: 80% utilization.
const KINK: Dec = match Dec::new(8, 1) {
    Some(kink) => kink,
    None => Dec::ZERO,
};
/// How steeply past the kink: 4.75 a unit of utilization.
const SLOPE: Dec = match Dec::new(475, 2) {
    Some(slope) => slope,
    None => Dec::ZERO,
};

/// The yearly borrow rate at `utilization`: `0.05 + 4.75 * max(0, u - 0.8)`,
/// the docs' rule for stablecoins. Every reserve on file, each under 80%,
/// charges the floor.
#[must_use]
pub fn borrow_rate(utilization: Dec) -> Dec {
    let past = utilization.checked_sub(KINK).unwrap_or(Dec::ZERO).max(Dec::ZERO);
    past.checked_mul(SLOPE).and_then(|steep| steep.checked_add(BASE_RATE)).unwrap_or(BASE_RATE)
}

/// What suppliers earn a year: the borrow rate times utilization, less the
/// protocol's share, to ten places.
#[must_use]
pub fn supply_rate(borrow_rate: Dec, utilization: Dec) -> Dec {
    let kept = Dec::ONE.checked_sub(PROTOCOL_SHARE).unwrap_or(Dec::ONE);
    borrow_rate
        .checked_mul(utilization)
        .and_then(|earned| earned.checked_mul(kept))
        .map_or(Dec::ZERO, |rate| rate.round(RATE_PLACES, Round::Nearest))
}

impl ReserveState {
    /// Borrowed over supplied, to ten places: what `utilization` should say.
    /// Zero while nothing is supplied.
    #[must_use]
    pub fn utilization_now(&self) -> Option<Dec> {
        match self.total_supplied.is_zero() {
            true => Some(Dec::ZERO),
            false => ratio(self.total_borrowed, self.total_supplied),
        }
    }

    /// [`supply_rate`] at this reserve's rate and utilization: what
    /// `supplyYearlyRate` should say.
    #[must_use]
    pub fn supply_rate_now(&self) -> Dec {
        supply_rate(self.borrow_yearly_rate, self.utilization)
    }

    /// What the reserve can still lend, in the token.
    #[must_use]
    pub const fn lendable(&self) -> Dec {
        self.balance
    }

    /// `amount` of the token in USDC at the oracle price.
    #[must_use]
    pub fn value_of(&self, amount: Dec) -> Option<Dec> {
        amount.checked_mul(self.oracle_px)
    }

    /// How much USDC `amount` of the token backs: its value at its loan to
    /// value. Zero for a token that backs nothing (USDC itself).
    #[must_use]
    pub fn backing(&self, amount: Dec) -> Option<Dec> {
        self.value_of(amount)?.checked_mul(self.ltv)
    }
}

/// `part / whole` to [`RATE_PLACES`], the nearest, both not negative. Both
/// are first counted in the same units: `Dec::checked_div` scales the
/// numerator by ten to the twentieth for two ten-place figures, which a
/// reserve's hundreds of millions overflow.
fn ratio(part: Dec, whole: Dec) -> Option<Dec> {
    let places = part.places().max(whole.places());
    let part = part.units_at(places, Round::Down)?;
    let whole = whole.units_at(places, Round::Down)?;
    if part < 0 || whole <= 0 {
        return None;
    }
    let scaled = part.checked_mul(10i128.checked_pow(RATE_PLACES)?)?;
    let (quotient, rest) = (scaled / whole, scaled % whole);
    let units = match rest.checked_mul(2)? >= whole {
        true => quotient.checked_add(1)?,
        false => quotient,
    };
    Dec::new(units, RATE_PLACES)
}

/// What an account can still borrow, in USDC: each held token's free amount
/// (`held`, by index: a spot balance less its hold) at its oracle price and
/// loan to value, less everything owed at its oracle price, at most what
/// USDC's reserve can lend and [`USER_BORROW_CAP`]. Nothing once the venue
/// calls the account liquidatable. Where the spot state gives USDC's
/// `tokenToAvailableAfterMaintenance`, that caps it too (atlas).
#[must_use]
pub fn borrowable(held: &[(u32, Dec)], reserves: &[(u32, ReserveState)], state: &UserState) -> Dec {
    let Some(usdc) = reserve(reserves, USDC_INDEX) else { return Dec::ZERO };
    if state.risk() == Risk::Liquidatable {
        return Dec::ZERO;
    }
    let backed = held
        .iter()
        .filter(|(token, free)| *token != USDC_INDEX && free.is_positive())
        .filter_map(|(token, free)| reserve(reserves, *token)?.backing(*free))
        .fold(Dec::ZERO, |sum, backing| sum.checked_add(backing).unwrap_or(sum));
    let owed = state
        .positions()
        .filter_map(|(token, position)| reserve(reserves, token)?.value_of(position.borrow.value))
        .fold(Dec::ZERO, |sum, value| sum.checked_add(value).unwrap_or(sum));
    backed.checked_sub(owed).unwrap_or(Dec::ZERO).min(usdc.lendable()).min(USER_BORROW_CAP).max(Dec::ZERO)
}

#[cfg(test)]
#[path = "tests/math.rs"]
mod tests;
