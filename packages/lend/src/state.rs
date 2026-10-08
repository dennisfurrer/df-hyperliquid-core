//! `allBorrowLendReserveStates`, `borrowLendReserveState` and
//! `borrowLendUserState`. Figures are the venue's decimal strings; rates and
//! utilization are fractions (0.05 is 5%). A token is its spot index, from the
//! caller's `spotMeta`: HYPE is 150 on mainnet and 1105 on testnet.

use hl_core::{Address, ApiError, Dec, Figure, Json, Transport, info, user_request};
use serde::Deserialize;

/// `{"type":"allBorrowLendReserveStates"}`.
#[must_use]
pub fn all_reserve_states_request() -> String {
    Json::Object(vec![("type".to_owned(), Json::String("allBorrowLendReserveStates".to_owned()))]).compact()
}

/// `{"type":"borrowLendReserveState","token":..}`. A token with no reserve is
/// answered HTTP 500.
#[must_use]
pub fn reserve_state_request(token: u32) -> String {
    Json::Object(vec![
        ("type".to_owned(), Json::String("borrowLendReserveState".to_owned())),
        ("token".to_owned(), Json::Number(token.to_string())),
    ])
    .compact()
}

/// `{"type":"borrowLendUserState","user":..}`.
#[must_use]
pub fn user_state_request(user: Address) -> String {
    user_request("borrowLendUserState", user)
}

/// Every reserve, as `[token index, state]` pairs.
pub async fn all_reserve_states(transport: &impl Transport) -> Result<Vec<(u32, ReserveState)>, ApiError> {
    info(transport, all_reserve_states_request()).await
}

/// One token's reserve.
pub async fn reserve_state(transport: &impl Transport, token: u32) -> Result<ReserveState, ApiError> {
    info(transport, reserve_state_request(token)).await
}

pub async fn user_state(transport: &impl Transport, user: Address) -> Result<UserState, ApiError> {
    info(transport, user_state_request(user)).await
}

/// One token's reserve.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveState {
    /// What a borrower pays a year.
    pub borrow_yearly_rate: Dec,
    /// What a supplier earns a year.
    pub supply_yearly_rate: Dec,
    /// The reserve's balance, in the token: what it can still lend. Live it is
    /// near, but not exactly, supplied less borrowed.
    pub balance: Dec,
    /// Borrowed over supplied.
    pub utilization: Dec,
    /// In USDC.
    pub oracle_px: Dec,
    /// How much of the token's value backs a borrow; 0 where it backs none.
    pub ltv: Dec,
    pub total_supplied: Dec,
    pub total_borrowed: Dec,
}

/// The reserve of `token` among `reserves`.
#[must_use]
pub fn reserve(reserves: &[(u32, ReserveState)], token: u32) -> Option<&ReserveState> {
    reserves.iter().find(|(index, _)| *index == token).map(|(_, reserve)| reserve)
}

/// An account's lending: a position per token it ever touched, and its
/// health.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    /// `[token index, position]` pairs; zero rows stay listed.
    #[serde(default)]
    pub token_to_state: Vec<(u32, Position)>,
    /// The venue's word: `healthy`, `atRisk` (seen live on a portfolio-margin
    /// account), or `liquidatable`.
    pub health: String,
    /// `null` while nothing is borrowed.
    #[serde(default)]
    pub health_factor: Option<Figure>,
}

/// How near liquidation the venue says an account's lending is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Risk {
    Safe,
    /// `atRisk`, or a word not seen yet: worth a warning.
    Near,
    Liquidatable,
}

impl UserState {
    #[must_use]
    pub fn position(&self, token: u32) -> Option<&Position> {
        self.token_to_state.iter().find(|(index, _)| *index == token).map(|(_, position)| position)
    }

    /// The positions that hold or owe anything, as hip4-tui lists them.
    pub fn positions(&self) -> impl Iterator<Item = (u32, &Position)> {
        self.token_to_state
            .iter()
            .filter(|(_, position)| !position.supply.value.is_zero() || !position.borrow.value.is_zero())
            .map(|(token, position)| (*token, position))
    }

    /// What is supplied of `token` now, interest in; zero for none.
    #[must_use]
    pub fn supplied(&self, token: u32) -> Dec {
        self.position(token).map_or(Dec::ZERO, |position| position.supply.value)
    }

    /// What is owed of `token` now, interest in; zero for none.
    #[must_use]
    pub fn borrowed(&self, token: u32) -> Dec {
        self.position(token).map_or(Dec::ZERO, |position| position.borrow.value)
    }

    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health == "healthy"
    }

    /// atlas's reading of the venue's word.
    #[must_use]
    pub fn risk(&self) -> Risk {
        match self.health.as_str() {
            "healthy" => Risk::Safe,
            "liquidatable" => Risk::Liquidatable,
            _ => Risk::Near,
        }
    }

    /// The health factor as written, while something is borrowed.
    #[must_use]
    pub fn factor(&self) -> Option<Dec> {
        self.health_factor.as_ref().and_then(Figure::to_dec)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub borrow: Basis,
    #[serde(default)]
    pub supply: Basis,
}

/// `value` is what is held or owed now, in the token; `basis` what went in.
/// A live portfolio-margin account wrote positions with no `basis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Basis {
    #[serde(default)]
    pub basis: Dec,
    pub value: Dec,
}

impl Basis {
    /// The interest earned or owed so far: the value less the basis.
    #[must_use]
    pub fn interest(&self) -> Option<Dec> {
        self.value.checked_sub(self.basis)
    }
}

#[cfg(test)]
#[path = "tests/lend.rs"]
mod tests;
