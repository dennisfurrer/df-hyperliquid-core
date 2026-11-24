//! Checks on a trading action that fail before anything is signed ([`TradeError`]).

use crate::trade::builder::OverCap;
use crate::utils::{BadUnits, Dec};

/// What would be refused, or would trade the wrong way, before anything is
/// signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TradeError {
    /// The slippage is not a fraction from 0 to 0.5; the slippage given.
    #[error("a slippage is a fraction from 0 to 0.5, which {0} is not")]
    Slippage(Dec),
    /// No valid price on the asset's grid comes from this price; the price given.
    #[error("{0} leaves no price on the asset's grid")]
    NoPrice(Dec),
    /// This size is zero once cut to the asset's size decimals; the size given.
    #[error("{0} leaves no size at the asset's size decimals")]
    NoSize(Dec),
    /// A TWAP's minutes are outside 5 to 1440; the minutes given.
    #[error("a TWAP runs 5 to 1440 minutes, not {0}")]
    TwapMinutes(u32),
    /// The leverage is 0.
    #[error("leverage is a whole number of at least 1")]
    NoLeverage,
    /// An isolated margin amount that is 0, finer than six places or too large.
    #[error("isolated margin moves a non-zero amount of USDC: {0}")]
    Margin(BadUnits),
    /// A builder fee over what the market allows.
    #[error(transparent)]
    OverCap(#[from] OverCap),
}
