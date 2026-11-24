//! `outcomeTemplates`: each template's id, role, title, rules and keywords, and how a row made from
//! one reads in `outcomeMeta`.

use std::{collections::BTreeMap, fmt};

use crate::client::{ApiError, Transport, info, info_request};
use crate::deploy::outcomes::actions::{Hint, Keywords};
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, SeqAccess, Visitor},
};

/// What a template makes, as `outcomeTemplates` says in its `role`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplateRole {
    /// `{"standaloneOutcome":{"sideNames"}}`: one outcome and its two sides.
    Standalone {
        /// The two sides' names (`sideNames`).
        side_names: Vec<String>,
    },
    /// `"question"`.
    Question,
    /// `{"questionOutcome":{"parent"}}`: a named outcome of a question made from `parent`.
    QuestionOutcome {
        /// The id of the question template it belongs to.
        parent: String,
    },
    /// A role this build does not know, kept by its name.
    Other(String),
}

/// One keyword a template declares, and the kind of value it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Keyword {
    /// The keyword, as it appears in braces in the title and rules.
    pub name: String,
    /// As `outcomeTemplates` spells it: `dateTime`, `hlPerp`.
    pub kind: String,
}

impl Keyword {
    /// How the keyword's value is written, read from [`kind`](Self::kind).
    #[must_use]
    pub fn hint(&self) -> Hint {
        Hint::parse(&self.kind)
    }
}

/// A template as `outcomeTemplates` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawTemplate")]
#[non_exhaustive]
pub struct OutcomeTemplate {
    /// The template's id, such as `binaryPrice`.
    pub id: String,
    /// What the template makes.
    pub role: TemplateRole,
    /// The title pattern, keywords in braces.
    pub name: String,
    /// The rules, keywords in braces.
    pub description: String,
    /// In the venue's order.
    pub keywords: Vec<Keyword>,
}

impl OutcomeTemplate {
    /// The question template a named outcome template belongs to.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        match &self.role {
            TemplateRole::QuestionOutcome { parent } => Some(parent),
            _ => None,
        }
    }

    /// What a row made from this template with `values` reads as in `outcomeMeta`:
    /// `template:<id>`, the keywords sorted as `key:value` with bars between, and a standalone
    /// outcome's side names under `template:`.
    #[must_use]
    pub fn on_chain(&self, values: &Keywords) -> OnChain {
        let mut pairs: Vec<(&str, &str)> = self
            .keywords
            .iter()
            .map(|keyword| (keyword.name.as_str(), values.get(&keyword.name).unwrap_or("")))
            .collect();
        pairs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let description = pairs.iter().map(|(key, value)| format!("{key}:{value}")).collect::<Vec<_>>().join("|");
        let sides = match &self.role {
            TemplateRole::Standalone { side_names } => match side_names.as_slice() {
                [first, second] => Some([format!("template:{first}"), format!("template:{second}")]),
                _ => None,
            },
            _ => None,
        };
        OnChain { name: format!("template:{}", self.id), description, sides }
    }
}

/// How the venue writes a row made from a template.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OnChain {
    /// The row's name: `template:<id>`.
    pub name: String,
    /// The row's description: `key:value` pairs sorted by key, joined by `|`.
    pub description: String,
    /// A standalone outcome's sides; a question's outcome has `Yes` and `No`.
    pub sides: Option<[String; 2]>,
}

#[derive(Deserialize)]
struct RawTemplate {
    #[serde(default)]
    id: String,
    #[serde(default)]
    role: RawRole,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    keywords: Vec<(String, String)>,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum RawRole {
    Named(String),
    Keyed(BTreeMap<String, RawRoleBody>),
    #[default]
    #[serde(skip)]
    Missing,
    Unknown(IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawRoleBody {
    Body {
        #[serde(default, rename = "sideNames")]
        side_names: Vec<String>,
        #[serde(default)]
        parent: String,
    },
    Unknown(IgnoredAny),
}

impl From<RawRole> for TemplateRole {
    fn from(raw: RawRole) -> TemplateRole {
        match raw {
            RawRole::Named(name) if name == "question" => TemplateRole::Question,
            RawRole::Named(name) => TemplateRole::Other(name),
            RawRole::Keyed(keyed) => {
                let Some((key, body)) = keyed.into_iter().next() else {
                    return TemplateRole::Other(String::new());
                };
                let (side_names, parent) = match body {
                    RawRoleBody::Body { side_names, parent } => (side_names, parent),
                    RawRoleBody::Unknown(_) => (Vec::new(), String::new()),
                };
                match key.as_str() {
                    "standaloneOutcome" => TemplateRole::Standalone { side_names },
                    "questionOutcome" => TemplateRole::QuestionOutcome { parent },
                    _ => TemplateRole::Other(key),
                }
            }
            RawRole::Missing | RawRole::Unknown(_) => TemplateRole::Other(String::new()),
        }
    }
}

impl From<RawTemplate> for OutcomeTemplate {
    fn from(raw: RawTemplate) -> OutcomeTemplate {
        OutcomeTemplate {
            id: raw.id,
            role: raw.role.into(),
            name: raw.name,
            description: raw.description,
            keywords: raw.keywords.into_iter().map(|(name, kind)| Keyword { name, kind }).collect(),
        }
    }
}

/// The `outcomeTemplates` answer. A template that does not read is skipped rather than failing the
/// rest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Templates(pub Vec<OutcomeTemplate>);

impl<'de> Deserialize<'de> for Templates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Lenient {
            Read(OutcomeTemplate),
            Skipped(IgnoredAny),
        }

        struct Each;

        impl<'de> Visitor<'de> for Each {
            type Value = Templates;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a list of templates")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Templates, A::Error> {
                let mut read = Vec::new();
                while let Some(one) = seq.next_element::<Lenient>()? {
                    if let Lenient::Read(template) = one {
                        read.push(template);
                    }
                }
                Ok(Templates(read))
            }
        }

        deserializer.deserialize_seq(Each)
    }
}

/// `{"type":"outcomeTemplates"}`.
#[must_use]
pub fn outcome_templates_request() -> String {
    info_request("outcomeTemplates", &[])
}

/// Every template the venue offers.
pub async fn outcome_templates(transport: &impl Transport) -> Result<Vec<OutcomeTemplate>, ApiError> {
    info::<Templates>(transport, outcome_templates_request()).await.map(|templates| templates.0)
}

#[cfg(test)]
#[path = "tests/listing.rs"]
mod tests;
