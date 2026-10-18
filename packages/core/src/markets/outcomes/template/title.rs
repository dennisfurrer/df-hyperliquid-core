//! A template's title and a side's name, written from a row's keywords.

use crate::markets::outcomes::fields::Fields;

/// `pattern` with each `{keyword}` replaced by its value, or by `?` where `value` gives none: a
/// template's title (`{perp} above {threshold} at {time}?`) written from a row's keywords.
#[must_use]
pub fn fill(pattern: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                out.push_str(&value(&after[..close]).unwrap_or_else(|| "?".to_owned()));
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// A side's name from its `sideSpecs` entry: `template:Yes` is `Yes`, `template:{shortNameA}` the
/// description's `shortNameA`, nothing at all `fallback`.
#[must_use]
pub fn label(spec: Option<&str>, fields: &Fields<'_>, fallback: &str) -> String {
    let raw = spec.unwrap_or(fallback).trim();
    let raw = raw.strip_prefix("template:").unwrap_or(raw);
    let Some(key) = raw.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) else {
        return match raw.is_empty() {
            true => fallback.to_owned(),
            false => raw.to_owned(),
        };
    };
    fields.text(key).unwrap_or_else(|| fallback.to_owned())
}

#[cfg(test)]
#[path = "tests/title.rs"]
mod tests;
