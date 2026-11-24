//! What an outcome action is refused for before it is signed: a bad venue name, a figure out of
//! range, an empty question or grant list, or a settlement `outcomeMeta` does not allow.

/// Why an action could not be made: what the venue would refuse, or what
/// would pay out the wrong way, before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DeployError {
    /// A venue name that is not 2 to 4 lower case letters; holds the name.
    #[error("`{0}` is no venue name: a venue is 2 to 4 lower case letters")]
    Venue(String),
    /// The name `spot`, which belongs to the spot venue.
    #[error("`spot` is the spot venue's name and cannot be claimed")]
    SpotVenue,
    /// A fee scale that is not a decimal from 0 to 10; holds the text given.
    #[error("a fee scale is a decimal from 0 to 10, which `{0}` is not")]
    FeeScale(String),
    /// A settle fraction that is not a decimal from 0 to 1; holds the text given.
    #[error("a settle fraction is a decimal from 0 to 1, which `{0}` is not")]
    Fraction(String),
    /// A question with no named outcomes.
    #[error("a question needs at least one named outcome")]
    NoNamedOutcomes,
    /// A `setSubDeployers` with no grants.
    #[error("setting sub-deployers needs at least one grant")]
    NoGrants,
    /// A question's outcome given a fraction other than 0 or 1; holds the outcome id.
    #[error("outcome {0} belongs to a question, which settles to 0 or 1 only")]
    NotWhole(u32),
    /// An outcome given twice in one settlement; holds the outcome id.
    #[error("outcome {0} is settled twice")]
    SettledTwice(u32),
    /// An outcome `outcomeMeta` does not list; holds the outcome id.
    #[error("outcome {0} is not in outcomeMeta")]
    NotListed(u32),
    /// An outcome whose `outcomeMeta` entry does not have two sides; holds the outcome id.
    #[error("outcome {0} does not list two sides")]
    NotTwoSided(u32),
    /// A question's fallback outcome settled on its own; it settles with its question.
    #[error("outcome {outcome} is question {question}'s fallback, which settles with its question")]
    Fallback {
        /// The fallback outcome's id.
        outcome: u32,
        /// The id of the question it is the fallback of.
        question: u32,
    },
    /// A question `outcomeMeta` does not list; holds the question id.
    #[error("question {0} is not in outcomeMeta")]
    NoQuestion(u32),
    /// An outcome that is not one of the question's named outcomes still left to settle.
    #[error("outcome {outcome} is no named outcome question {question} has left to settle")]
    NotOpen {
        /// The outcome's id.
        outcome: u32,
        /// The question's id.
        question: u32,
    },
}
