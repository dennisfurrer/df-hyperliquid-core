use crate::trade::order::OverCap;
use crate::utils::{BadUnits, Dec};

/// What would be refused, or would trade the wrong way, before anything is
/// signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TradeError {
    #[error("a slippage is a fraction from 0 to 0.5, which {0} is not")]
    Slippage(Dec),
    #[error("{0} leaves no price on the asset's grid")]
    NoPrice(Dec),
    #[error("{0} leaves no size at the asset's size decimals")]
    NoSize(Dec),
    #[error("a TWAP runs 5 to 1440 minutes, not {0}")]
    TwapMinutes(u32),
    #[error("leverage is a whole number of at least 1")]
    NoLeverage,
    #[error("isolated margin moves a non-zero amount of USDC: {0}")]
    Margin(BadUnits),
    #[error(transparent)]
    OverCap(#[from] OverCap),
}
