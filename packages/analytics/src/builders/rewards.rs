//! What a builder code has been paid, all time, as `referral` answers for the builder's address
//! (hl-account's [`Referral`]): builder fees and referral rewards in one pot, unclaimed and
//! claimed, in USDC. The builder fees are named apart; the rest of the pot is what the address's
//! own referral code earned. Rewards in another quote token are not in these figures.

use hl_account::{Referral, ReferrerState};
use hl_core::Dec;

/// Where the address stands as a referrer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Stage {
    /// It has a code and earns on it.
    Ready { code: String },
    /// It has traded enough and has made no code.
    NeedsCode,
    /// It has to trade `required` more before it may make a code.
    NeedsVolume { required: Dec },
    /// A stage this build does not know, by its name.
    Other(String),
    /// Nothing read.
    #[default]
    Unread,
}

impl Stage {
    #[must_use]
    pub fn of(state: &ReferrerState) -> Stage {
        let data = state.data.as_ref();
        match state.stage.as_str() {
            "ready" => Stage::Ready { code: data.and_then(|data| data.code.clone()).unwrap_or_default() },
            "needToCreateCode" => Stage::NeedsCode,
            "needToTrade" => Stage::NeedsVolume { required: data.and_then(|data| data.required).unwrap_or(Dec::ZERO) },
            other => Stage::Other(other.to_owned()),
        }
    }

    /// Whether the address earns on a referral code.
    #[must_use]
    pub const fn earning(&self) -> bool {
        matches!(self, Stage::Ready { .. })
    }
}

/// One address's pot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rewards {
    /// Builder fees, claimed or not.
    pub builder: Dec,
    pub unclaimed: Dec,
    pub claimed: Dec,
    /// What the address has traded, all time.
    pub volume: Dec,
    pub stage: Stage,
}

impl Rewards {
    #[must_use]
    pub fn of(referral: &Referral) -> Rewards {
        Rewards {
            builder: referral.builder_rewards,
            unclaimed: referral.unclaimed_rewards,
            claimed: referral.claimed_rewards,
            volume: referral.cum_vlm,
            stage: Stage::of(&referral.referrer_state),
        }
    }

    /// Everything paid in, claimed or not.
    #[must_use]
    pub fn pot(&self) -> Dec {
        self.unclaimed.checked_add(self.claimed).unwrap_or(self.unclaimed)
    }

    /// The pot less the builder fees: what the address's own code earned. Never below zero.
    #[must_use]
    pub fn referral(&self) -> Dec {
        self.pot().checked_sub(self.builder).unwrap_or(Dec::ZERO).max(Dec::ZERO)
    }

    /// While a code is out of reach: what has been traded, and the volume a code takes.
    #[must_use]
    pub fn toward_code(&self) -> Option<(Dec, Dec)> {
        match &self.stage {
            Stage::NeedsVolume { required } => Some((self.volume, self.volume.checked_add(*required)?)),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/rewards.rs"]
mod tests;
