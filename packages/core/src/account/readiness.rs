//! Whether a builder can be paid, and what an order naming one needs first. An order over the fee
//! the account approved asks for an approval first; one whose approval did not read is refused
//! rather than guessed at.

use std::fmt;

use crate::account::builder::ApproveBuilderFee;
use crate::account::state::{Abstraction, PerpState};
use crate::account::{abstraction, perp_state};
use crate::client::{ApiError, Transport};
use crate::trade::builder_fee::BuilderFee;
use crate::utils::{Address, Dec};

/// The least perps account value a builder needs, in USDC.
pub const BUILDER_MINIMUM: Dec = Dec::from_int(100);

/// The mode a builder needs: standard as set (`disabled`), not `default`, a mode never set.
pub const BUILDER_MODE: Abstraction = Abstraction::Disabled;

/// Whether a builder can be paid now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    /// Not ready, and every reason.
    NotReady(Vec<NotReady>),
}

impl Readiness {
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Readiness::Ready)
    }
}

/// One reason a builder is not ready.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotReady {
    #[error("{value} USDC perps account value, under the {BUILDER_MINIMUM} a builder needs")]
    TooLittle { value: Dec },
    #[error("its account abstraction is {0}, and a builder needs standard")]
    NotStandard(Abstraction),
}

/// Ready or not, from the builder's main dex (`marginSummary.accountValue`)
/// and its `userAbstraction`.
#[must_use]
pub fn readiness(perp: &PerpState, mode: &Abstraction) -> Readiness {
    let value = perp.margin_summary.account_value;
    let mut why = Vec::new();
    if value < BUILDER_MINIMUM {
        why.push(NotReady::TooLittle { value });
    }
    if *mode != BUILDER_MODE {
        why.push(NotReady::NotStandard(mode.clone()));
    }
    match why.is_empty() {
        true => Readiness::Ready,
        false => Readiness::NotReady(why),
    }
}

/// Reads the builder's main dex and its mode, one after the other, and
/// judges them. Weighs 22: 2 for `clearinghouseState`, 20 for
/// `userAbstraction`.
pub async fn builder_readiness(transport: &impl Transport, builder: Address) -> Result<Readiness, ApiError> {
    let perp = perp_state(transport, builder).await?;
    let mode = abstraction(transport, builder).await?;
    Ok(readiness(&perp, &mode))
}

/// What the account has approved its builder to charge, as `maxBuilderFee`
/// last answered, or as the last approval the venue took.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Approved {
    /// Nothing read yet.
    #[default]
    Unread,
    Upto(BuilderFee),
    /// The read failed, and why in words.
    Failed(String),
}

impl Approved {
    /// What a read of `maxBuilderFee` leaves known.
    #[must_use]
    pub fn after(read: Result<BuilderFee, ApiError>) -> Approved {
        match read {
            Ok(fee) => Approved::Upto(fee),
            Err(error) => Approved::Failed(error.to_string()),
        }
    }

    /// An approval the venue took for `builder` is what the account has
    /// approved from then on, lower or higher; one for another builder
    /// changes nothing.
    #[must_use]
    pub fn taken(self, builder: Address, approval: &ApproveBuilderFee) -> Approved {
        match (approval.builder() == builder, approval.max_fee()) {
            (true, Some(fee)) => Approved::Upto(fee),
            _ => self,
        }
    }

    /// For display: `up to 0.88% (88 bps)`.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Approved::Unread => "not read yet".to_owned(),
            Approved::Upto(fee) if *fee == BuilderFee::ZERO => "nothing yet".to_owned(),
            Approved::Upto(fee) => format!("up to {} ({} bps)", fee.percent(), bps(*fee)),
            Approved::Failed(why) => format!("would not read: {why}"),
        }
    }
}

/// Tenths of a basis point as basis points: 88 is 8.8.
fn bps(fee: BuilderFee) -> String {
    Dec::lit::<1>(i128::from(fee.tenths())).to_wire()
}

/// What an order paying a builder needs before it is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clearance {
    /// Nothing: no builder, a fee of 0, or a fee already approved.
    Clear,
    /// An approval first: the account has approved `approved`, under the fee.
    Approve { approved: BuilderFee },
    /// Refused: what the account has approved is not known.
    Refused(String),
}

impl fmt::Display for Clearance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clearance::Clear => f.write_str("clear"),
            Clearance::Approve { approved } => write!(f, "needs an approval over the {approved} approved"),
            Clearance::Refused(why) => f.write_str(why),
        }
    }
}

/// What an order paying `fee` to its builder needs, by what the account has
/// approved. A fee of 0 needs no approval; an approval not read, or that
/// would not read, refuses the order rather than guess at it.
#[must_use]
pub fn clearance(approved: &Approved, fee: Option<BuilderFee>) -> Clearance {
    let Some(fee) = fee.filter(|fee| *fee > BuilderFee::ZERO) else {
        return Clearance::Clear;
    };
    match approved {
        Approved::Upto(approved) if *approved >= fee => Clearance::Clear,
        Approved::Upto(approved) => Clearance::Approve { approved: *approved },
        Approved::Unread => Clearance::Refused(
            "the builder fee this account has approved is not read yet; try again in a moment".to_owned(),
        ),
        Approved::Failed(why) => Clearance::Refused(format!(
            "the builder fee this account has approved would not read, so it is not guessed at: {why}"
        )),
    }
}

#[cfg(test)]
#[path = "tests/readiness.rs"]
mod tests;
