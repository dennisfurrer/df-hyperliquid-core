//! What a spot deploy, perp deploy or aligned quote action is refused for before it is signed: a
//! name, figure or list the docs rule out.

/// Why a `spotDeploy`, `perpDeploy`, `validatorL1Stream` or `requestEvmContract` action could not
/// be made: a value outside what the venue's docs allow, caught before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDeploy {
    /// A dex name or deployer label that is not 2 to 4 lower case letters.
    #[error("`{0}` is no dex name or deployer label: those are 2 to 4 lower case letters")]
    Name(String),
    /// A deployer trading fee share that is not a percentage from 0% to 100%.
    #[error("a deployer trading fee share is a percentage from 0% to 100%, such as `99.4%`, which `{0}` is not")]
    FeeShare(String),
    /// A decimal outside the range the docs allow.
    #[error("{what} is a decimal from {least} to {most}, which `{value}` is not")]
    OutOfRange {
        /// What the value is, such as `a funding clamp`.
        what: &'static str,
        /// The lowest value allowed, as text.
        least: &'static str,
        /// The highest value allowed, as text; `any` when there is no upper limit.
        most: &'static str,
        /// The value given, as it would be sent.
        value: String,
    },
    /// A text longer than the docs allow.
    #[error("{what} is at most {most} characters, which `{value}` is over")]
    TooLong {
        /// What the text is, such as `a keyword`.
        what: &'static str,
        /// The most characters allowed.
        most: usize,
        /// The text given.
        value: String,
    },
    /// An annotation with more than 2 keywords; holds how many were given.
    #[error("an annotation takes at most 2 keywords, not {0}")]
    Keywords(usize),
    /// A coin given twice in one per-coin list; holds the coin.
    #[error("`{0}` is listed twice")]
    Twice(String),
    /// A `setOracle` with more than 2 lists of mark prices; holds how many were given.
    #[error("setOracle takes at most 2 lists of mark prices, not {0}")]
    MarkPxs(usize),
    /// A margin table with no tiers, more than 3, a leverage outside 1 to 50, or tiers out of order.
    #[error("a margin table takes 1 to 3 tiers, lower bounds rising, max leverage falling and each from 1 to 50")]
    MarginTiers,
    /// A `setMarginTableIds` that gives a coin table id 0; holds the coin.
    #[error("`{0}` is given margin table 0, and a margin table id is never 0")]
    MarginTableZero(String),
    /// An `evmExtraWeiDecimals` outside -2 to 18; holds the value given.
    #[error("evmExtraWeiDecimals is from -2 to 18, not {0}")]
    EvmExtraWeiDecimals(i8),
    /// A `setSubDeployers` with no grants.
    #[error("setting sub-deployers needs at least one grant")]
    NoGrants,
}
