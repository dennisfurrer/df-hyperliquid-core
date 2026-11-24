//! Why a type, a signature, a value or a word is refused.

/// Why an ABI type, signature, value or encoded word is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AbiError {
    /// A type name that does not parse, as written.
    #[error("`{0}` is not a type this reads")]
    Type(String),
    /// A function or event signature that does not parse as `name(types)`, as written.
    #[error("`{0}` is not a signature this reads; expected name(types)")]
    Signature(String),
    /// A value written as text that does not parse as its type or is out of its range.
    #[error("{name} must be {wants}, and `{got}` is not")]
    Value {
        /// What the caller calls the field.
        name: String,
        /// What the type accepts, in words.
        wants: String,
        /// The text that was given, trimmed.
        got: String,
    },
    /// The number of values given differs from the number of parameters.
    #[error("expected {wants} values and there are {got}")]
    Count {
        /// The number of parameters.
        wants: usize,
        /// The number of values given.
        got: usize,
    },
    /// A token that does not fit its parameter's type, such as a string for a `uint256`.
    #[error("a {kind} cannot hold {token}")]
    Mismatch {
        /// The declared type, canonically written.
        kind: String,
        /// The token that was given, described in words, such as `the number 5` or `a string`.
        token: String,
    },
    /// The data ends at this byte length, before all of the declared types are read.
    #[error("the data ends at byte {0}, before the shape it was declared with")]
    Short(usize),
    /// A 32-byte word whose value does not fit the type it is read as, such as a `bool` that is not 0 or 1.
    #[error("the word at byte {at} does not fit {kind}")]
    Misfit {
        /// The byte offset of the word in the data.
        at: usize,
        /// The type the word was read as, canonically written, or `a length` for a length or offset.
        kind: String,
    },
    /// Calldata that starts with a different 4-byte function selector.
    #[error("the calldata is for selector {found}, not {wants}")]
    Selector {
        /// The function's selector, as `0x` hex.
        wants: String,
        /// The selector the calldata starts with, as `0x` hex.
        found: String,
    },
}
