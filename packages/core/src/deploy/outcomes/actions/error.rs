//! What an outcome action is refused for before it is signed: a bad venue name, a figure out of
//! range, an empty question or grant list, or a settlement `outcomeMeta` does not allow.

/// Why an action could not be made: what the venue would refuse, or what
/// would pay out the wrong way, before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeployError {
    #[error("`{0}` is no venue name: a venue is 2 to 4 lower case letters")]
    Venue(String),
    #[error("`spot` is the spot venue's name and cannot be claimed")]
    SpotVenue,
    #[error("a fee scale is a decimal from 0 to 10, which `{0}` is not")]
    FeeScale(String),
    #[error("a settle fraction is a decimal from 0 to 1, which `{0}` is not")]
    Fraction(String),
    #[error("a question needs at least one named outcome")]
    NoNamedOutcomes,
    #[error("setting sub-deployers needs at least one grant")]
    NoGrants,
    #[error("outcome {0} belongs to a question, which settles to 0 or 1 only")]
    NotWhole(u32),
    #[error("outcome {0} is settled twice")]
    SettledTwice(u32),
    #[error("outcome {0} is not in outcomeMeta")]
    NotListed(u32),
    #[error("outcome {0} does not list two sides")]
    NotTwoSided(u32),
    #[error("outcome {outcome} is question {question}'s fallback, which settles with its question")]
    Fallback { outcome: u32, question: u32 },
    #[error("question {0} is not in outcomeMeta")]
    NoQuestion(u32),
    #[error("outcome {outcome} is no named outcome question {question} has left to settle")]
    NotOpen { outcome: u32, question: u32 },
}
