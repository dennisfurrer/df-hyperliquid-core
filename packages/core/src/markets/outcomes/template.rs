//! Templates: an outcome's name (`template:<id><version>`) gives its [`Family`]; `outcomeTemplates`
//! lists each template's title, roles and keywords, and how a row made from one reads
//! ([`OutcomeTemplate::on_chain`]); [`fill`] and [`label`] write a title and a side's name from the
//! keywords; `outcomeDeployerLimits` says what a venue may still register.

use std::{collections::BTreeMap, fmt};

use crate::client::{ApiError, Transport, info, info_request};
use crate::deploy::outcomes::actions::{Hint, Keywords};
use crate::markets::outcomes::fields::Fields;
use crate::utils::Json;
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, SeqAccess, Visitor},
};

/// Which way a policy rate moves, for the three named outcomes of a rate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RateMove {
    Increase,
    Decrease,
    NoChange,
}

/// What kind of market a template makes, read from its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    BinaryPrice,
    PriceTouch,
    ScalarPrice,
    ContestWinner,
    ContestParticipant,
    ContestDraw,
    TournamentParticipant,
    SportsScalar,
    SportsOverUnder,
    PolicyRate(RateMove),
    /// The venue's own daily markets, named `Recurring`.
    Recurring,
    Fallback,
    /// A deployer's own name, no template.
    Named,
    ContestResult,
    TournamentWinner,
    PolicyRateDecision,
    /// A venue template with no family here; its `outcomeTemplates` entry says what it makes.
    Template,
}

impl Family {
    /// The families that describe a question rather than a book.
    #[must_use]
    pub const fn is_question(self) -> bool {
        matches!(self, Family::ContestResult | Family::TournamentWinner | Family::PolicyRateDecision)
    }
}

/// An outcome's name read: its family and the version digits after the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    pub family: Family,
    pub version: Option<u8>,
}

impl Template {
    /// Reads `template:<stem><N>`, a fallback (`template fallback`, testnet's `Fallback`,
    /// `Recurring Fallback`), a `Recurring` name, or a deployer's own name.
    #[must_use]
    pub fn classify(name: &str) -> Template {
        let name = name.trim();
        let fallback = ["template fallback", "fallback", "recurring fallback"];
        if fallback.iter().any(|spelling| name.eq_ignore_ascii_case(spelling)) {
            return Template { family: Family::Fallback, version: None };
        }
        let Some(rest) = name.strip_prefix("template:") else {
            let family = if name.starts_with("Recurring") { Family::Recurring } else { Family::Named };
            return Template { family, version: None };
        };
        let stem = rest.trim_end_matches(|c: char| c.is_ascii_digit());
        let digits = &rest[stem.len()..];
        let family = match stem {
            "binaryPrice" => Family::BinaryPrice,
            "priceTouch" => Family::PriceTouch,
            "scalePrice" | "scalarPrice" => Family::ScalarPrice,
            "sportsContestWinner" => Family::ContestWinner,
            "sportsContestParticipant" => Family::ContestParticipant,
            "sportsContestDraw" => Family::ContestDraw,
            "sportsTournamentParticipant" => Family::TournamentParticipant,
            "sportsScalarMarket" => Family::SportsScalar,
            "sportsOverUnderMarket" => Family::SportsOverUnder,
            "policyRateIncrease" => Family::PolicyRate(RateMove::Increase),
            "policyRateDecrease" => Family::PolicyRate(RateMove::Decrease),
            "policyRateNoChange" => Family::PolicyRate(RateMove::NoChange),
            "sportsContestResult" => Family::ContestResult,
            "sportsTournamentWinner" => Family::TournamentWinner,
            "policyRateDecision" => Family::PolicyRateDecision,
            _ => Family::Template,
        };
        Template { family, version: digits.parse().ok() }
    }
}

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

/// What a template makes, as `outcomeTemplates` says in its `role`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRole {
    /// `{"standaloneOutcome":{"sideNames"}}`: one outcome and its two sides.
    Standalone { side_names: Vec<String> },
    /// `"question"`.
    Question,
    /// `{"questionOutcome":{"parent"}}`: a named outcome of a question made from `parent`.
    QuestionOutcome { parent: String },
    /// A role this build does not know, kept by its name.
    Other(String),
}

/// One keyword a template declares, and the kind of value it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub name: String,
    /// As `outcomeTemplates` spells it: `dateTime`, `hlPerp`.
    pub kind: String,
}

impl Keyword {
    #[must_use]
    pub fn hint(&self) -> Hint {
        Hint::parse(&self.kind)
    }
}

/// A template as `outcomeTemplates` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawTemplate")]
pub struct OutcomeTemplate {
    pub id: String,
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
pub struct OnChain {
    pub name: String,
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

pub async fn outcome_templates(transport: &impl Transport) -> Result<Vec<OutcomeTemplate>, ApiError> {
    info::<Templates>(transport, outcome_templates_request()).await.map(|templates| templates.0)
}

/// `{"type":"outcomeDeployerLimits","venue":..}`.
#[must_use]
pub fn deployer_limits_request(venue: &str) -> String {
    info_request("outcomeDeployerLimits", &[("venue", Json::String(venue.to_owned()))])
}

/// What `outcomeDeployerLimits` answers: how many more a venue may register today and how many
/// more it may hold live. Neither cap is documented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Remaining {
    #[serde(default, rename = "nDailyOutcomesRemaining")]
    pub daily: u32,
    #[serde(default, rename = "nActiveOutcomesRemaining")]
    pub active: u32,
}

/// How many more outcomes `venue` may register today and hold live.
pub async fn deployer_limits(transport: &impl Transport, venue: &str) -> Result<Remaining, ApiError> {
    info(transport, deployer_limits_request(venue)).await
}

#[cfg(test)]
#[path = "tests/template.rs"]
mod tests;
