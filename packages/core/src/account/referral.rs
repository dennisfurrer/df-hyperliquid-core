//! Referrals: `setReferrer`, the L1 action that gives an account a referrer,
//! and the `referral` read. The venue keeps the first code an account sets.

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// The longest code `setReferrer` takes.
pub const LONGEST_CODE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a referral code is 1 to {LONGEST_CODE} characters, which `{0}` is not")]
pub struct BadCode(pub String);

/// `setReferrer`: `{type, code}`. An agent may sign it for its master.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetReferrer {
    code: String,
}

impl SetReferrer {
    pub fn new(code: &str) -> Result<SetReferrer, BadCode> {
        match (1..=LONGEST_CODE).contains(&code.chars().count()) {
            true => Ok(SetReferrer { code: code.to_owned() }),
            false => Err(BadCode(code.to_owned())),
        }
    }
}

impl L1Action for SetReferrer {
    fn action(&self) -> Map {
        Map::new().put("type", "setReferrer").put("code", self.code.clone())
    }
}

/// `{"type":"referral","user":..}`.
#[must_use]
pub fn referral_request(user: Address) -> String {
    user_request("referral", user)
}

pub async fn referral(transport: &impl Transport, user: Address) -> Result<Referral, ApiError> {
    info(transport, referral_request(user)).await
}

/// Who referred an account, and its own standing as a referrer. The top
/// level's figures are USDC's; each quote token's are in `token_to_state`.
/// Reward history is left unread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Referral {
    #[serde(default)]
    pub referred_by: Option<ReferredBy>,
    pub cum_vlm: Dec,
    pub unclaimed_rewards: Dec,
    pub claimed_rewards: Dec,
    #[serde(default)]
    pub builder_rewards: Dec,
    pub referrer_state: ReferrerState,
    /// Each quote token's figures, by the token's index (`0` USDC); not always sent.
    #[serde(default)]
    pub token_to_state: Option<Vec<(u32, TokenRewards)>>,
}

/// What an account traded and was paid in one quote token.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRewards {
    pub cum_vlm: Dec,
    pub unclaimed_rewards: Dec,
    pub claimed_rewards: Dec,
    pub builder_rewards: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferredBy {
    pub referrer: Address,
    pub code: String,
}

/// `stage` is `needToTrade` (with `required`, the volume still to trade),
/// `needToCreateCode`, or `ready` (with the account's own `code`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferrerState {
    pub stage: String,
    #[serde(default)]
    pub data: Option<ReferrerData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferrerData {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub required: Option<Dec>,
    #[serde(default)]
    pub n_referrals: Option<u64>,
    /// The accounts referred under the code.
    #[serde(default)]
    pub referral_states: Vec<ReferralState>,
}

/// One account referred under the code. Per-token figures are left unread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferralState {
    pub user: Address,
    #[serde(default)]
    pub cum_vlm: Dec,
    #[serde(default)]
    pub cum_rewarded_fees_since_referred: Dec,
    #[serde(default)]
    pub cum_fees_rewarded_to_referrer: Dec,
    /// Milliseconds.
    #[serde(default)]
    pub time_joined: u64,
}

impl Referral {
    /// One quote token's figures, by its index.
    #[must_use]
    pub fn token(&self, index: u32) -> Option<&TokenRewards> {
        self.token_to_state.as_deref()?.iter().find(|(token, _)| *token == index).map(|(_, state)| state)
    }

    /// The account's own code, once it has made one.
    #[must_use]
    pub fn own_code(&self) -> Option<&str> {
        self.referrer_state.data.as_ref()?.code.as_deref()
    }

    /// The code the account trades under, if any.
    #[must_use]
    pub fn referred_code(&self) -> Option<&str> {
        self.referred_by.as_ref().map(|by| by.code.as_str())
    }

    /// Who owns the code the account trades under.
    #[must_use]
    pub fn referrer(&self) -> Option<Address> {
        self.referred_by.as_ref().map(|by| by.referrer)
    }

    /// Whether the account owns `code`, and so cannot be referred under it.
    /// Codes are compared without case.
    #[must_use]
    pub fn owns(&self, code: &str) -> bool {
        self.own_code().is_some_and(|own| own.eq_ignore_ascii_case(code))
    }
}

#[cfg(test)]
#[path = "tests/referral.rs"]
mod tests;
