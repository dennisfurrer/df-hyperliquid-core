//! A template instance's values, kept in the order the venue hashes them, each checked by the rules
//! every value keeps and by its keyword's [`Hint`]. What the venue would refuse comes back as a
//! [`BadValue`] naming its [`Problem`].

pub mod hint;
pub mod problem;

pub use hint::{HORIZON_MS, Hint, MAX_SHORT_LEN};
pub use problem::{BadValue, MAX_VALUE_LEN, Problem};

use std::collections::{BTreeMap, btree_map::Entry};

use crate::deploy::outcomes::actions::keywords::problem::plain;
use crate::utils::sign::Value;

/// A template instance's `keywordToValue`: one value per keyword, kept in the
/// order the venue hashes them, which is bytewise by keyword, so `Zone` comes
/// before `apple`. Built sorted, so no caller can send them otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keywords(BTreeMap<String, String>);

impl Keywords {
    /// No keywords, which a template that declares none is instanced with.
    #[must_use]
    pub const fn new() -> Self {
        Keywords(BTreeMap::new())
    }

    /// Pairs in any order. A keyword given twice is refused, not replaced.
    pub fn from_pairs<K: Into<String>, V: Into<String>>(
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self, BadValue> {
        pairs.into_iter().try_fold(Keywords::new(), |keywords, (keyword, value)| keywords.with(keyword, value))
    }

    /// One more pair. A keyword given twice is refused, not replaced.
    pub fn with(mut self, keyword: impl Into<String>, value: impl Into<String>) -> Result<Self, BadValue> {
        let keyword = keyword.into();
        let value = value.into();
        plain(&keyword, &value)?;
        match self.0.entry(keyword) {
            Entry::Occupied(taken) => Err(BadValue::new(taken.key(), Problem::Repeated)),
            Entry::Vacant(free) => {
                free.insert(value);
                Ok(self)
            }
        }
    }

    #[must_use]
    pub fn get(&self, keyword: &str) -> Option<&str> {
        self.0.get(keyword).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The pairs in the order they are hashed in.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(keyword, value)| (keyword.as_str(), value.as_str()))
    }

    /// `[[keyword, value], ...]`, or `[]` for none.
    #[must_use]
    pub fn wire(&self) -> Value {
        Value::array(self.iter().map(|(keyword, value)| Value::array([keyword.to_owned(), value.to_owned()])))
    }

    /// Everything wrong with these as an instance of a template declaring
    /// `declared`, each keyword with its hint: a declared keyword with no
    /// value, a value for a keyword it does not declare, and each value its
    /// hint refuses at `now_ms`. Empty when nothing is.
    #[must_use]
    pub fn check<'a>(&self, declared: impl IntoIterator<Item = (&'a str, Hint)>, now_ms: u64) -> Vec<BadValue> {
        let declared: BTreeMap<&str, Hint> = declared.into_iter().collect();
        let mut problems = Vec::new();
        for (keyword, hint) in &declared {
            match self.get(keyword) {
                None => problems.push(BadValue::new(keyword, Problem::Missing)),
                Some(value) => problems.extend(hint.check_at(keyword, value, now_ms).err()),
            }
        }
        for keyword in self.0.keys() {
            if !declared.contains_key(keyword.as_str()) {
                problems.push(BadValue::new(keyword, Problem::Unknown));
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "tests/keywords.rs"]
mod tests;
