//! How a template says a keyword's value is written, and what each hint checks on top of the rules
//! every value keeps; a `dateTime` is also checked against a clock the caller passes.

use std::fmt;

use crate::deploy::outcomes::actions::keywords::problem::{BadValue, Problem, plain};
use crate::utils::time::Stamp;

/// The longest value a `shortString` keyword takes, counted as [`MAX_VALUE_LEN`](super::MAX_VALUE_LEN) is.
pub const MAX_SHORT_LEN: usize = 10;

/// How far ahead a `dateTime` value may point: 366 days, in milliseconds.
pub const HORIZON_MS: u64 = 366 * 24 * 60 * 60 * 1000;

/// `0`, or digits that do not start with a zero.
fn is_uint(text: &str) -> bool {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    digits && (text == "0" || !text.starts_with('0'))
}

/// A `uInt`, then optionally a point and digits that do not end in a zero.
fn is_udecimal(text: &str) -> bool {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    is_uint(whole)
        && fraction.is_none_or(|fraction| {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit()) && !fraction.ends_with('0')
        })
}

/// How a template says a keyword's value is written, by the name
/// `outcomeTemplates` gives next to each keyword.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Hint {
    /// `YYYYMMDD-HHMM` in UTC, after now and within a year of it.
    DateTime,
    /// `YYYYMMDD`.
    Date,
    /// At most [`MAX_SHORT_LEN`] bytes.
    ShortString,
    /// A perp's name. Whether one by that name trades is for the caller to
    /// check against the live universe; here it only has to be present.
    HlPerp,
    /// `0`, or digits with no leading zero, below 2^64.
    UInt,
    /// A `uInt`, then optionally a point and digits not ending in a zero.
    UDecimal,
    /// Free text.
    String,
    /// A hint this build does not know, checked as free text and kept under
    /// its own name.
    Other(String),
}

impl Hint {
    /// Never fails: a name this build does not know becomes [`Hint::Other`].
    #[must_use]
    pub fn parse(name: &str) -> Hint {
        match name {
            "dateTime" => Hint::DateTime,
            "date" => Hint::Date,
            "shortString" => Hint::ShortString,
            "hlPerp" => Hint::HlPerp,
            "uInt" => Hint::UInt,
            "uDecimal" => Hint::UDecimal,
            "string" => Hint::String,
            other => Hint::Other(other.to_owned()),
        }
    }

    /// As `outcomeTemplates` spells it.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Hint::DateTime => "dateTime",
            Hint::Date => "date",
            Hint::ShortString => "shortString",
            Hint::HlPerp => "hlPerp",
            Hint::UInt => "uInt",
            Hint::UDecimal => "uDecimal",
            Hint::String => "string",
            Hint::Other(name) => name,
        }
    }

    /// Every rule for `value` that needs no clock. A `dateTime` is only
    /// checked for its shape here; [`Hint::check_at`] adds when it may be.
    pub fn check(&self, keyword: &str, value: &str) -> Result<(), BadValue> {
        plain(keyword, value)?;
        let problem = match self {
            Hint::DateTime if Stamp::exact(value).is_none() => Problem::DateTime,
            Hint::Date if Stamp::day(value).is_none() => Problem::Date,
            Hint::ShortString if value.len() > MAX_SHORT_LEN => Problem::ShortString,
            Hint::UInt if !is_uint(value) => Problem::UInt,
            Hint::UInt if value.parse::<u64>().is_err() => Problem::TooLarge,
            Hint::UDecimal if !is_udecimal(value) => Problem::UDecimal,
            _ => return Ok(()),
        };
        Err(BadValue::new(keyword, problem))
    }

    /// [`Hint::check`], and a `dateTime` after `now_ms` and at most
    /// [`HORIZON_MS`] after it, as the venue asks. A `date` has no rule on
    /// when it may be.
    pub fn check_at(&self, keyword: &str, value: &str, now_ms: u64) -> Result<(), BadValue> {
        self.check(keyword, value)?;
        if *self != Hint::DateTime {
            return Ok(());
        }
        let Some(stamp) = Stamp::exact(value) else {
            return Err(BadValue::new(keyword, Problem::DateTime));
        };
        let at = i128::from(stamp.ms());
        let now = i128::from(now_ms);
        let problem = if at <= now {
            Problem::Passed
        } else if at > now + i128::from(HORIZON_MS) {
            Problem::TooFar
        } else {
            return Ok(());
        };
        Err(BadValue::new(keyword, problem))
    }
}

impl fmt::Display for Hint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
#[path = "tests/hint.rs"]
mod tests;
