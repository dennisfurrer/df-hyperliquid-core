//! Builder fees as a setting: what each market kind pays the caller's builder, held to that kind's
//! cap, and whether the account has approved that much. Whether the builder can be paid at all is
//! [`crate::account::builder_readiness`].

use crate::trade::builder_fee::BuilderFee;
use crate::trade::order::{Builder, OverCap, PlaceOrder};
use crate::utils::{Address, Asset, AssetKind};

/// What each order pays the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeRule {
    /// This many percent of each market kind's cap, cut down to a whole tenth of a basis point: 88 is
    /// 88 of a perp's 100 and 880 of spot's 1000. At most 100.
    ShareOfCap(u64),
    /// One fee on every market; an order whose market allows less is refused,
    /// never lowered.
    Flat(BuilderFee),
}

/// The builder orders are routed through, and what they pay it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderSetting {
    address: Address,
    rule: FeeRule,
}

impl BuilderSetting {
    /// `percent` of each market's cap; `None` over 100.
    #[must_use]
    pub const fn share_of_cap(address: Address, percent: u64) -> Option<BuilderSetting> {
        match percent <= 100 {
            true => Some(BuilderSetting { address, rule: FeeRule::ShareOfCap(percent) }),
            false => None,
        }
    }

    /// `fee` on every market.
    #[must_use]
    pub const fn flat(address: Address, fee: BuilderFee) -> BuilderSetting {
        BuilderSetting { address, rule: FeeRule::Flat(fee) }
    }

    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    #[must_use]
    pub const fn rule(&self) -> FeeRule {
        self.rule
    }

    /// What an order on `kind` pays, whether the market allows it or not.
    #[must_use]
    pub const fn fee_on(&self, kind: AssetKind) -> BuilderFee {
        match self.rule {
            FeeRule::ShareOfCap(percent) => {
                BuilderFee::from_tenths(kind.builder_cap().tenths().saturating_mul(percent) / 100)
            }
            FeeRule::Flat(fee) => fee,
        }
    }

    /// The builder an order on `kind` names, or why it cannot.
    pub fn on(&self, kind: AssetKind) -> Result<Builder, OverCap> {
        let fee = self.fee_on(kind);
        let cap = kind.builder_cap();
        match fee > cap {
            true => Err(OverCap { fee, cap, kind }),
            false => Ok(Builder::new(self.address, fee)),
        }
    }

    /// The builder one action over these assets names: held to the
    /// strictest kind among them, so one perp order holds the whole action
    /// to the perp cap. `None` for no assets.
    pub fn on_assets(&self, assets: impl IntoIterator<Item = Asset>) -> Result<Option<Builder>, OverCap> {
        AssetKind::strictest(assets).map(|kind| self.on(kind)).transpose()
    }

    /// The most an order on any market it may trade pays, which is what one
    /// approval has to cover: 880 at 88% of each cap.
    #[must_use]
    pub fn most(&self) -> BuilderFee {
        AssetKind::ALL
            .into_iter()
            .filter_map(|kind| self.on(kind).ok())
            .map(|builder| builder.fee())
            .max()
            .unwrap_or_default()
    }

    /// What `order` needs before it is sent, by what the account has approved this builder to charge
    /// and by who signs.
    pub fn approval(&self, order: &PlaceOrder, approved: &Approved, by: SignedBy) -> Result<Approval, ApprovalRefused> {
        let Some(builder) = order.builder().filter(|builder| builder.fee() > BuilderFee::ZERO) else {
            return Ok(Approval::Goes);
        };
        if builder.address() != self.address {
            return Err(ApprovalRefused::OtherBuilder { named: builder.address(), set: self.address });
        }
        let fee = builder.fee();
        let approved = match approved {
            Approved::Upto(approved) if *approved >= fee => return Ok(Approval::Goes),
            Approved::Upto(approved) => *approved,
            Approved::Unread => return Err(ApprovalRefused::NotRead),
            Approved::Failed(why) => return Err(ApprovalRefused::Unreadable(why.clone())),
        };
        let rate = self.most().max(fee);
        match by {
            SignedBy::Account => Ok(Approval::ApproveFirst { approved, rate }),
            SignedBy::Agent => Err(ApprovalRefused::AgentCannotApprove { approved, rate }),
        }
    }
}

/// What the account has approved its builder to charge, as `maxBuilderFee`
/// last answered or as the last approval the venue took.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Approved {
    /// Nothing read yet.
    #[default]
    Unread,
    /// Up to this.
    Upto(BuilderFee),
    /// The read failed, and why.
    Failed(String),
}

impl Approved {
    /// What the account has approved, in words.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Approved::Unread => "not read yet".to_owned(),
            Approved::Upto(fee) if *fee == BuilderFee::ZERO => "nothing yet".to_owned(),
            Approved::Upto(fee) => format!("up to {fee}"),
            Approved::Failed(why) => format!("would not read: {why}"),
        }
    }
}

/// Whose key signs the order. Only the account's own may sign
/// `approveBuilderFee`; an agent's may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedBy {
    Account,
    Agent,
}

/// What an order paying a builder needs before it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// Nothing: no builder, a fee of 0, or a fee already approved.
    Goes,
    /// `approveBuilderFee` at `rate` first, signed by the account: it has
    /// approved `approved`, under the order's fee. `rate` is the most any
    /// market pays, so one approval covers them all.
    ApproveFirst { approved: BuilderFee, rate: BuilderFee },
}

impl Approval {
    /// The `maxFeeRate` `approveBuilderFee` names: `0.88%` for 880.
    #[must_use]
    pub fn max_fee_rate(&self) -> Option<String> {
        match self {
            Approval::Goes => None,
            Approval::ApproveFirst { rate, .. } => Some(rate.percent()),
        }
    }
}

/// Why an order cannot go for its builder fee: what is not known is refused
/// rather than guessed at.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalRefused {
    #[error("the builder fee this account has approved is not read yet; try again in a moment")]
    NotRead,
    #[error("the builder fee this account has approved would not read, so it is not guessed at: {0}")]
    Unreadable(String),
    #[error(
        "the account's main key has to approve the builder fee up to {rate} once (approved now: {approved}); an agent key cannot sign approveBuilderFee"
    )]
    AgentCannotApprove { approved: BuilderFee, rate: BuilderFee },
    #[error("the order names the builder {named}, and what the account approved was read for {set}")]
    OtherBuilder { named: Address, set: Address },
}

#[cfg(test)]
#[path = "tests/builder.rs"]
mod tests;
