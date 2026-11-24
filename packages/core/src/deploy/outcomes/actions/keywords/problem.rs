//! What may be wrong with one keyword's value, in words for the person typing it, and the rules every
//! value keeps whatever its hint.

use std::fmt;

/// The longest value a keyword takes, in bytes; whether the venue counts bytes or characters is not confirmed.
pub const MAX_VALUE_LEN: usize = 100;

/// What is wrong with one keyword's value, in words for the person typing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Problem {
    /// A declared keyword with no value, or an empty value.
    Missing,
    /// A value over [`MAX_VALUE_LEN`] bytes.
    TooLong,
    /// `{`, `}` or `|`, which would break the `keyword:value|...` description
    /// the venue writes from the values.
    Reserved,
    /// A `dateTime` value not written `YYYYMMDD-HHMM`.
    DateTime,
    /// A `date` value not written `YYYYMMDD`.
    Date,
    /// A `shortString` value over [`MAX_SHORT_LEN`](super::MAX_SHORT_LEN) bytes.
    ShortString,
    /// A `uInt` value with a sign, a leading zero or anything but digits.
    UInt,
    /// A `uInt` of 2^64 or more.
    TooLarge,
    /// A `uDecimal` value with a sign, an exponent, or a leading or trailing zero.
    UDecimal,
    /// A `dateTime` at or before now.
    Passed,
    /// A `dateTime` more than [`HORIZON_MS`](super::HORIZON_MS) after now.
    TooFar,
    /// A value for a keyword the template does not declare.
    Unknown,
    /// A keyword given a value twice.
    Repeated,
}

impl Problem {
    /// Said after the keyword's name: `time takes YYYYMMDD-HHMM in UTC`.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Problem::Missing => "needs a value",
            Problem::TooLong => "takes at most 100 characters",
            Problem::Reserved => "cannot hold {, } or |",
            Problem::DateTime => "takes YYYYMMDD-HHMM in UTC",
            Problem::Date => "takes YYYYMMDD in UTC",
            Problem::ShortString => "takes at most 10 characters",
            Problem::UInt => "takes a whole number with no sign or leading zeros",
            Problem::TooLarge => "takes a whole number that fits in 64 bits",
            Problem::UDecimal => "takes a decimal with no sign, no exponent and no leading or trailing zeros",
            Problem::Passed => "has already passed",
            Problem::TooFar => "is more than a year away",
            Problem::Unknown => "is not a keyword of this template",
            Problem::Repeated => "is given twice",
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.words())
    }
}

/// A keyword's value the venue would refuse, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{keyword} {problem}")]
pub struct BadValue {
    /// The keyword whose value is refused.
    pub keyword: String,
    /// What is wrong with the value.
    pub problem: Problem,
}

impl BadValue {
    pub(super) fn new(keyword: &str, problem: Problem) -> Self {
        BadValue { keyword: keyword.to_owned(), problem }
    }
}

/// What every value must be, whatever its hint: present, at most
/// [`MAX_VALUE_LEN`] bytes, and free of `{`, `}` and `|`.
pub(super) fn plain(keyword: &str, value: &str) -> Result<(), BadValue> {
    let problem = if value.is_empty() {
        Problem::Missing
    } else if value.len() > MAX_VALUE_LEN {
        Problem::TooLong
    } else if value.contains(['{', '}', '|']) {
        Problem::Reserved
    } else {
        return Ok(());
    };
    Err(BadValue::new(keyword, problem))
}
