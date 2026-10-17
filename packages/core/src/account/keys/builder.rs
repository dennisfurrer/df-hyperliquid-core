//! Builder fees from the account's side: `approveBuilderFee`, what an account has approved a
//! builder to charge (`maxBuilderFee`), and whether a builder can be paid at all. An order whose
//! builder fee is over the approval is refused by the venue; what an order needs approved first is
//! [`crate::trade::BuilderSetting::approval`].

use crate::account::holdings::perps::{ClearinghouseState, clearinghouse_state};
use crate::account::settings::mode::{Abstraction, abstraction};
use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::Dex;
use crate::trade::builder::fee::BuilderFee;
use crate::utils::sign::{Field, NonceKey, UserAction, UserSigned};
use crate::utils::{Address, Dec, Json};

/// The least perps account value a builder needs, in USDC.
pub const BUILDER_MINIMUM: Dec = Dec::from_int(100);

/// The mode a builder needs: standard as set (`disabled`), not `default`, a mode never set.
pub const BUILDER_MODE: Abstraction = Abstraction::Disabled;

/// `approveBuilderFee`, signed by the account's own wallet: lets `builder`
/// charge up to `max_fee` on this account's orders. `maxFeeRate` is the
/// percent string (880 tenths is `0.88%`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveBuilderFee {
    max_fee_rate: String,
    builder: Address,
    nonce: u64,
}

impl ApproveBuilderFee {
    #[must_use]
    pub fn new(builder: Address, max_fee: BuilderFee, nonce: u64) -> ApproveBuilderFee {
        ApproveBuilderFee { max_fee_rate: max_fee.percent(), builder, nonce }
    }

    /// `maxFeeRate` as posted.
    #[must_use]
    pub fn max_fee_rate(&self) -> &str {
        &self.max_fee_rate
    }

    /// `maxFeeRate` back in tenths of a basis point.
    #[must_use]
    pub fn max_fee(&self) -> Option<BuilderFee> {
        BuilderFee::from_percent(&self.max_fee_rate)
    }

    #[must_use]
    pub const fn builder(&self) -> Address {
        self.builder
    }
}

impl UserAction for ApproveBuilderFee {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("approveBuilderFee", "HyperliquidTransaction:ApproveBuilderFee", NonceKey::Nonce, self.nonce)
            .field("maxFeeRate", Field::String(&self.max_fee_rate))
            .field("builder", Field::Address(self.builder))
    }
}

/// `{"type":"maxBuilderFee","user":..,"builder":..}`.
#[must_use]
pub fn max_builder_fee_request(user: Address, builder: Address) -> String {
    info_request("maxBuilderFee", &[("user", Json::String(user.to_hex())), ("builder", Json::String(builder.to_hex()))])
}

/// What `user` has approved `builder` to charge, in tenths of a basis point;
/// 0 for a builder never approved.
pub async fn max_builder_fee(
    transport: &impl Transport,
    user: Address,
    builder: Address,
) -> Result<BuilderFee, ApiError> {
    info(transport, max_builder_fee_request(user, builder)).await
}

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
pub fn readiness(perp: &ClearinghouseState, mode: &Abstraction) -> Readiness {
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
    let perp = clearinghouse_state(transport, builder, Dex::Main).await?;
    let mode = abstraction(transport, builder).await?;
    Ok(readiness(&perp, &mode))
}

#[cfg(test)]
#[path = "tests/builder.rs"]
mod tests;
