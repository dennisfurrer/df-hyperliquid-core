//! `allBorrowLendReserveStates` and `borrowLendUserState`. Figures are the
//! venue's decimal strings; rates and utilization are fractions (0.05 is 5%).

use hl_core::{Address, ApiError, Dec, Figure, Json, Transport, info, user_request};
use serde::Deserialize;

/// `{"type":"allBorrowLendReserveStates"}`.
#[must_use]
pub fn all_reserve_states_request() -> String {
    Json::Object(vec![("type".to_owned(), Json::String("allBorrowLendReserveStates".to_owned()))]).compact()
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
    /// The reserve's balance, in the token. Live it is near, but not exactly,
    /// supplied less borrowed.
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

/// An account's lending: a position per token it ever touched, and its
/// health.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    /// `[token index, position]` pairs; zero rows stay listed.
    #[serde(default)]
    pub token_to_state: Vec<(u32, Position)>,
    /// `healthy` on every account read so far.
    pub health: String,
    /// `null` while nothing is borrowed.
    #[serde(default)]
    pub health_factor: Option<Figure>,
}

impl UserState {
    #[must_use]
    pub fn position(&self, token: u32) -> Option<&Position> {
        self.token_to_state.iter().find(|(index, _)| *index == token).map(|(_, position)| position)
    }

    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health == "healthy"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Position {
    pub borrow: Basis,
    pub supply: Basis,
}

/// `value` is what is held or owed now, in the token; `basis` what went in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Basis {
    pub basis: Dec,
    pub value: Dec,
}

#[cfg(test)]
#[path = "tests/lend.rs"]
mod tests;
