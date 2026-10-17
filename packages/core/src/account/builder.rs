//! Builder fees: `approveBuilderFee`, and what an account has approved a
//! builder to charge (`maxBuilderFee`). An order whose builder fee is over
//! the approval is refused by the venue.

use crate::client::{ApiError, Transport, info, info_request};
use crate::trade::builder::fee::BuilderFee;
use crate::utils::sign::{Field, NonceKey, UserAction, UserSigned};
use crate::utils::{Address, Json};

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
