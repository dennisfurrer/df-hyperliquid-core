//! Referrals: `setReferrer`, the L1 action that gives an account a referrer, `claimRewards`, which
//! pays out what its referrals have earned it, and the `referral` read. The venue keeps the first
//! code an account sets.

use crate::account::{ActionKind, Signers};
use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// The longest code `setReferrer` takes.
pub const LONGEST_CODE: usize = 20;

/// A code [`SetReferrer::new`] refused, kept as given.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a referral code is 1 to {LONGEST_CODE} characters, which `{0}` is not")]
pub struct BadCode(pub String);

/// `setReferrer`: `{type, code}`. An agent may sign it for its master.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetReferrer {
    code: String,
}

impl SetReferrer {
    /// Refers the account under `code`: 1 to [`LONGEST_CODE`] characters.
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

/// `claimRewards`: `{type}` alone. Pays the account its unclaimed referral rewards
/// ([`TokenRewards::unclaimed_rewards`]). An agent may sign it for its master.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClaimRewards;

impl ClaimRewards {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::ClaimRewards.signers();
}

impl L1Action for ClaimRewards {
    fn action(&self) -> Map {
        Map::new().put("type", "claimRewards")
    }
}

/// `{"type":"referral","user":..}`.
#[must_use]
pub fn referral_request(user: Address) -> String {
    user_request("referral", user)
}

/// Reads `referral`; see [`referral_request`].
pub async fn referral(transport: &impl Transport, user: Address) -> Result<Referral, ApiError> {
    info(transport, referral_request(user)).await
}

/// Who referred an account, and its own standing as a referrer. The top
/// level's figures are USDC's; each quote token's are in `token_to_state`.
/// Reward history is left unread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Referral {
    /// Who referred the account (`referredBy`); `None` when no one did.
    #[serde(default)]
    pub referred_by: Option<ReferredBy>,
    /// The account's own volume (`cumVlm`), in USDC.
    pub cum_vlm: Dec,
    /// Referral rewards not yet claimed (`unclaimedRewards`), in USDC.
    pub unclaimed_rewards: Dec,
    /// Referral rewards already claimed (`claimedRewards`), in USDC.
    pub claimed_rewards: Dec,
    /// What the account has earned as a builder (`builderRewards`), in USDC; zero when left out.
    #[serde(default)]
    pub builder_rewards: Dec,
    /// Where the account stands as a referrer (`referrerState`).
    pub referrer_state: ReferrerState,
    /// Each quote token's figures, by the token's index (`0` USDC); not always sent.
    #[serde(default)]
    pub token_to_state: Option<Vec<(u32, TokenRewards)>>,
}

/// What an account traded and was paid in one quote token.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRewards {
    /// The account's own volume in this token (`cumVlm`).
    pub cum_vlm: Dec,
    /// Referral rewards in this token not yet claimed (`unclaimedRewards`).
    pub unclaimed_rewards: Dec,
    /// Referral rewards in this token already claimed (`claimedRewards`).
    pub claimed_rewards: Dec,
    /// What the account has earned in this token as a builder (`builderRewards`).
    pub builder_rewards: Dec,
}

/// Who referred an account, and under which code.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferredBy {
    /// The address that owns the code.
    pub referrer: Address,
    /// The code the account was referred under.
    pub code: String,
}

/// `stage` is `needToTrade` (with `required`, the volume still to trade),
/// `needToCreateCode`, or `ready` (with the account's own `code`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReferrerState {
    /// `needToTrade`, `needToCreateCode` or `ready`.
    pub stage: String,
    /// The stage's figures; `None` when the stage has none.
    #[serde(default)]
    pub data: Option<ReferrerData>,
}

/// What a [`ReferrerState`] stage carries: the code once made, or the volume still to trade.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferrerData {
    /// The account's own code (`code`); `None` until it has made one.
    #[serde(default)]
    pub code: Option<String>,
    /// The volume still to trade before a code can be made (`required`), in USDC; `None` once
    /// traded.
    #[serde(default)]
    pub required: Option<Dec>,
    /// How many accounts were referred under the code (`nReferrals`); `None` when not sent.
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
    /// The referred account's address.
    pub user: Address,
    /// The referred account's volume (`cumVlm`), in USDC.
    #[serde(default)]
    pub cum_vlm: Dec,
    /// The referred account's fees counted for rewards since it was referred
    /// (`cumRewardedFeesSinceReferred`), in USDC.
    #[serde(default)]
    pub cum_rewarded_fees_since_referred: Dec,
    /// What the referred account's fees have paid the referrer (`cumFeesRewardedToReferrer`), in
    /// USDC.
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
