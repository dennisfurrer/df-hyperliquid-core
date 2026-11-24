//! What frontends show for a spot token or a HIP-3 perp: the text `setTokenAnnotation` and
//! `setPerpAnnotation` carry, checked against the lengths the docs give.

use crate::deploy::error::BadDeploy;
use crate::utils::sign::{Map, Value};

/// The longest category, in characters.
pub const MAX_CATEGORY_LEN: usize = 15;
/// The longest description, in characters.
pub const MAX_DESCRIPTION_LEN: usize = 400;
/// The longest display name, in characters.
pub const MAX_DISPLAY_NAME_LEN: usize = 9;
/// The most keywords an annotation takes.
pub const MAX_KEYWORDS: usize = 2;
/// The longest keyword, in characters.
pub const MAX_KEYWORD_LEN: usize = 10;

/// A token's or perp's annotation: a category, a description, a display name
/// frontends show instead of the listed name (`None` to keep the listed
/// name) and up to two keywords searches match against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    category: String,
    description: String,
    display_name: Option<String>,
    keywords: Vec<String>,
}

/// `text` if it is at most `most` characters.
fn short(what: &'static str, text: String, most: usize) -> Result<String, BadDeploy> {
    if text.chars().count() > most {
        return Err(BadDeploy::TooLong { what, most, value: text });
    }
    Ok(text)
}

impl Annotation {
    /// Refuses a category over 15 characters, a description over 400, a
    /// display name over 9, more than 2 keywords or a keyword over 10.
    pub fn new(
        category: impl Into<String>,
        description: impl Into<String>,
        display_name: Option<String>,
        keywords: Vec<String>,
    ) -> Result<Self, BadDeploy> {
        if keywords.len() > MAX_KEYWORDS {
            return Err(BadDeploy::Keywords(keywords.len()));
        }
        Ok(Annotation {
            category: short("a category", category.into(), MAX_CATEGORY_LEN)?,
            description: short("a description", description.into(), MAX_DESCRIPTION_LEN)?,
            display_name: display_name.map(|name| short("a display name", name, MAX_DISPLAY_NAME_LEN)).transpose()?,
            keywords: keywords
                .into_iter()
                .map(|keyword| short("a keyword", keyword, MAX_KEYWORD_LEN))
                .collect::<Result<_, _>>()?,
        })
    }

    /// The category, at most 15 characters.
    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    /// The description, at most 400 characters.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The name frontends show instead of the listed name; `None` keeps the listed name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Up to two words searches match against, each at most 10 characters.
    #[must_use]
    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }

    /// `{"category","description","displayName","keywords"}` after the keys
    /// already in `map`; a missing display name is sent as null.
    #[must_use]
    pub fn wire_after(&self, map: Map) -> Map {
        let display_name = self.display_name.clone().map_or(Value::Nil, Value::from);
        map.put("category", self.category.clone())
            .put("description", self.description.clone())
            .put("displayName", display_name)
            .put("keywords", Value::array(self.keywords.iter().cloned()))
    }
}

#[cfg(test)]
#[path = "tests/annotation.rs"]
mod tests;
