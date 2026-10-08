//! An outcome's or a question's description read as the venue writes it from
//! a template: `keyword:value` pairs between bars, a value free to hold
//! colons. Prose reads as no pairs at all.

use crate::time::Stamp;

/// The pairs of one description, in its order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Fields<'a>(Vec<(&'a str, &'a str)>);

impl<'a> Fields<'a> {
    /// Keeps a segment whose key is letters and digits and whose value is not
    /// empty; anything else, prose included, is dropped.
    #[must_use]
    pub fn parse(description: &'a str) -> Self {
        let pairs = description
            .split('|')
            .filter_map(|segment| {
                let (key, value) = segment.split_once(':')?;
                let key = key.trim();
                let named = !key.is_empty() && key.chars().all(char::is_alphanumeric);
                (named && !value.is_empty()).then_some((key, value))
            })
            .collect();
        Fields(pairs)
    }

    /// Every keyword and its value, in the order the description gives them.
    pub fn pairs(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The value as written, spaces and all.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&'a str> {
        self.0.iter().find_map(|(name, value)| (*name == key).then_some(*value))
    }

    /// The value trimmed; `None` when nothing is left.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        self.get(key).map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
    }

    /// A number exactly as the chain wrote it, `100000.00` included: it is
    /// shown, never computed with, so it is never re-encoded.
    #[must_use]
    pub fn number(&self, key: &str) -> Option<String> {
        numeric(self.get(key)?).map(str::to_owned)
    }

    /// A comma-separated list of numbers, each as written.
    #[must_use]
    pub fn numbers(&self, key: &str) -> Vec<String> {
        self.get(key)
            .into_iter()
            .flat_map(|value| value.split(','))
            .filter_map(|part| numeric(part).map(str::to_owned))
            .collect()
    }

    /// A whole number: a window in seconds, a bucket's index.
    #[must_use]
    pub fn seconds(&self, key: &str) -> Option<u32> {
        self.get(key)?.trim().parse().ok()
    }

    #[must_use]
    pub fn time(&self, key: &str) -> Option<Stamp> {
        Stamp::parse(self.get(key)?)
    }
}

/// `text` trimmed when it is a decimal with an optional sign and no exponent.
fn numeric(value: &str) -> Option<&str> {
    let text = value.trim();
    let body = text.strip_prefix('-').or_else(|| text.strip_prefix('+')).unwrap_or(text);
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let shaped = whole.len() + fraction.len() > 0
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit());
    shaped.then_some(text)
}

#[cfg(test)]
#[path = "tests/fields.rs"]
mod tests;
