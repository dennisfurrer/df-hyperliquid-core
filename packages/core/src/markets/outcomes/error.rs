//! What an outcome order is refused for before it is signed: a coin with no asset id, or a price,
//! size, amount or slippage out of range.

use crate::markets::outcomes::coin::Coin;
use crate::utils::Dec;

/// What would be refused before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OutcomeError {
    /// The side's outcome is too large to have an asset id; holds the coin.
    #[error("outcome {0} has no asset id")]
    NoAsset(Coin),
    /// A price off the grid or outside 0.00001 to 0.99999; holds the price.
    #[error("{0} is not an outcome price: 0.00001 to 0.99999 on a 0.00001 grid")]
    Price(Dec),
    /// A size of zero contracts, or not a whole number; holds the size.
    #[error("{0} is not a whole number of contracts above zero")]
    Size(Dec),
    /// A quote amount of zero or less, or with more than eight places; holds the amount.
    #[error("{0} is not an amount of the quote token above zero with at most eight places")]
    Amount(Dec),
    /// A slippage outside 0 to 0.5; holds the slippage.
    #[error("a slippage is a fraction from 0 to 0.5, which {0} is not")]
    Slippage(Dec),
}
