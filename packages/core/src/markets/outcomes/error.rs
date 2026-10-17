use crate::markets::outcomes::coin::Coin;
use crate::utils::Dec;

/// What would be refused before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutcomeError {
    #[error("outcome {0} has no asset id")]
    NoAsset(Coin),
    #[error("{0} is not an outcome price: 0.00001 to 0.99999 on a 0.00001 grid")]
    Price(Dec),
    #[error("{0} is not a whole number of contracts above zero")]
    Size(Dec),
    #[error("{0} is not an amount of the quote token above zero with at most eight places")]
    Amount(Dec),
    #[error("a slippage is a fraction from 0 to 0.5, which {0} is not")]
    Slippage(Dec),
}
