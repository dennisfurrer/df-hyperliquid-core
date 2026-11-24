//! What a spot deploy, perp deploy or aligned quote action is refused for before it is signed: a
//! name, figure or list the docs rule out.

/// Why a `spotDeploy`, `perpDeploy`, `validatorL1Stream` or `requestEvmContract` action could not
/// be made: a value outside what the venue's docs allow, caught before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDeploy {
    #[error("`{0}` is no dex name or deployer label: those are 2 to 4 lower case letters")]
    Name(String),
    #[error("a deployer trading fee share is a percentage from 0% to 100%, such as `99.4%`, which `{0}` is not")]
    FeeShare(String),
    #[error("{what} is a decimal from {least} to {most}, which `{value}` is not")]
    OutOfRange { what: &'static str, least: &'static str, most: &'static str, value: String },
    #[error("{what} is at most {most} characters, which `{value}` is over")]
    TooLong { what: &'static str, most: usize, value: String },
    #[error("an annotation takes at most 2 keywords, not {0}")]
    Keywords(usize),
    #[error("`{0}` is listed twice")]
    Twice(String),
    #[error("setOracle takes at most 2 lists of mark prices, not {0}")]
    MarkPxs(usize),
    #[error("a margin table takes 1 to 3 tiers, lower bounds rising, max leverage falling and each from 1 to 50")]
    MarginTiers,
    #[error("`{0}` is given margin table 0, and a margin table id is never 0")]
    MarginTableZero(String),
    #[error("evmExtraWeiDecimals is from -2 to 18, not {0}")]
    EvmExtraWeiDecimals(i8),
    #[error("setting sub-deployers needs at least one grant")]
    NoGrants,
}
