//! A template instance: a template's id and a value for each of its keywords, which every register
//! is made from.

use crate::deploy::outcomes::actions::bounded::FeeScale;
use crate::deploy::outcomes::actions::keywords::Keywords;
use crate::utils::sign::Map;

/// A template and a value for each of its keywords: `{"id","keywordToValue"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// The template's id in `outcomeTemplates`, such as `binaryPrice`.
    pub id: String,
    /// A value for each of the template's keywords (`keywordToValue`).
    pub keywords: Keywords,
}

impl Instance {
    /// An instance of the template `id` with `keywords`.
    #[must_use]
    pub fn new(id: impl Into<String>, keywords: Keywords) -> Self {
        Instance { id: id.into(), keywords }
    }

    /// `{"id","keywordToValue"}`, with no fee scale.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("id", self.id.clone()).put("keywordToValue", self.keywords.wire())
    }

    /// With the fee scale a standalone outcome or a question carries, last.
    pub(super) fn wire_with(&self, fee_scale: FeeScale) -> Map {
        self.wire().put("deployerFeeScale", fee_scale.to_wire())
    }
}
