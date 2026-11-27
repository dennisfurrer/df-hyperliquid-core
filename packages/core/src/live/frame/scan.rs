//! A cursor over a frame's text for the fixed layouts the venue writes ([`super::hot`]). Each step
//! reads exactly one expected token and answers `None` for anything else: whitespace, an escape in
//! a string, a number with a sign, a fraction or a leading zero. `None` sends the frame to the
//! serde reader, which reads every valid frame; so this cursor never has to read every valid
//! spelling, only to read the ones it takes the way serde_json would.

use std::str::FromStr;

use crate::utils::dec::Dec;

pub(super) struct Scan<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Scan<'a> {
    pub(super) const fn new(text: &'a str) -> Self {
        Scan { text, at: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        self.text.as_bytes().get(self.at..).unwrap_or_default()
    }

    /// Whether the whole text has been read.
    pub(super) fn done(&self) -> bool {
        self.at == self.text.len()
    }

    /// Reads `literal` if the text goes on with it.
    pub(super) fn eat(&mut self, literal: &str) -> bool {
        let found = self.rest().starts_with(literal.as_bytes());
        if found {
            self.at += literal.len();
        }
        found
    }

    /// [`Scan::eat`], as an `Option` for `?`.
    pub(super) fn expect(&mut self, literal: &str) -> Option<()> {
        self.eat(literal).then_some(())
    }

    /// The next byte, not read.
    pub(super) fn peek(&self) -> Option<u8> {
        self.rest().first().copied()
    }

    /// A JSON string with no escapes and no control characters, without its quotes.
    pub(super) fn string(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        if rest.first() != Some(&b'"') {
            return None;
        }
        let len = rest.iter().skip(1).position(|&byte| byte == b'"' || byte == b'\\' || byte < 0x20)?;
        if rest.get(1 + len) != Some(&b'"') {
            return None;
        }
        // The quotes are ASCII, so both ends are character boundaries.
        let read = self.text.get(self.at + 1..self.at + 1 + len)?;
        self.at += len + 2;
        Some(read)
    }

    /// A JSON integer with no sign and no leading zero that fits in a `u64`.
    pub(super) fn uint(&mut self) -> Option<u64> {
        let rest = self.rest();
        let len = rest.iter().position(|byte| !byte.is_ascii_digit()).unwrap_or(rest.len());
        let digits = rest.get(..len)?;
        if digits.is_empty() || (digits.len() > 1 && digits.first() == Some(&b'0')) {
            return None;
        }
        // A fraction or an exponent after the digits is not an integer.
        if matches!(rest.get(len), Some(b'.' | b'e' | b'E')) {
            return None;
        }
        let mut value: u64 = 0;
        for &digit in digits {
            value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
        }
        self.at += len;
        Some(value)
    }

    /// A decimal written as a JSON string, read by [`Dec::parse`].
    pub(super) fn dec(&mut self) -> Option<Dec> {
        Dec::parse(self.string()?).ok()
    }

    /// A JSON string read by `T`'s [`FromStr`].
    pub(super) fn parsed<T: FromStr>(&mut self) -> Option<T> {
        self.string()?.parse().ok()
    }
}

#[cfg(test)]
#[path = "tests/scan.rs"]
mod tests;
