//! Templates: what an outcome's name says about it, and what the venue
//! publishes in `outcomeTemplates`.
//!
//! An outcome made from a template is named `template:<id><version>`, so the
//! name alone gives its [`Family`] and the description its values. A template
//! this build has no family for is read from the [`Pattern`] the venue
//! publishes for it. `outcomeDeployerLimits` says what a venue may still
//! register; its cap is the live count plus what remains, never a constant.

use std::{collections::BTreeMap, fmt, str::FromStr};

use hl_core::{ApiError, Json, Transport, info};
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, SeqAccess, Visitor},
};

use crate::deploy::Hint;

/// Which way a policy rate moves, for the three named outcomes of a rate
/// decision.
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
    /// A venue template this build has no family for, read from the pattern
    /// the venue publishes for it.
    Template,
}

impl Family {
    /// Every family, in reporting order.
    pub const ALL: [Family; 19] = [
        Family::BinaryPrice,
        Family::PriceTouch,
        Family::ScalarPrice,
        Family::ContestWinner,
        Family::ContestParticipant,
        Family::ContestDraw,
        Family::TournamentParticipant,
        Family::SportsScalar,
        Family::SportsOverUnder,
        Family::PolicyRate(RateMove::Increase),
        Family::PolicyRate(RateMove::Decrease),
        Family::PolicyRate(RateMove::NoChange),
        Family::Recurring,
        Family::Fallback,
        Family::Named,
        Family::ContestResult,
        Family::TournamentWinner,
        Family::PolicyRateDecision,
        Family::Template,
    ];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Family::BinaryPrice => "binary_price",
            Family::PriceTouch => "price_touch",
            Family::ScalarPrice => "scalar_price",
            Family::ContestWinner => "contest_winner",
            Family::ContestParticipant => "contest_participant",
            Family::ContestDraw => "contest_draw",
            Family::TournamentParticipant => "tournament_participant",
            Family::SportsScalar => "sports_scalar",
            Family::SportsOverUnder => "sports_over_under",
            Family::PolicyRate(RateMove::Increase) => "policy_rate_increase",
            Family::PolicyRate(RateMove::Decrease) => "policy_rate_decrease",
            Family::PolicyRate(RateMove::NoChange) => "policy_rate_no_change",
            Family::Recurring => "recurring",
            Family::Fallback => "fallback",
            Family::Named => "named",
            Family::ContestResult => "contest_result",
            Family::TournamentWinner => "tournament_winner",
            Family::PolicyRateDecision => "policy_rate_decision",
            Family::Template => "template",
        }
    }

    /// The families that describe a question rather than a book.
    #[must_use]
    pub const fn is_question(self) -> bool {
        matches!(self, Family::ContestResult | Family::TournamentWinner | Family::PolicyRateDecision)
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown kind `{0}`")]
pub struct UnknownFamily(pub String);

impl FromStr for Family {
    type Err = UnknownFamily;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wanted = text.trim();
        Family::ALL.into_iter().find(|family| family.slug() == wanted).ok_or_else(|| UnknownFamily(wanted.to_owned()))
    }
}

/// An outcome's name read: its family and the version digits after the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    pub family: Family,
    pub version: Option<u8>,
}

impl Template {
    /// Reads `template:<stem><N>`, a fallback (`template fallback`, testnet's
    /// `Fallback`, `Recurring Fallback`), a `Recurring` name, or a deployer's
    /// own name.
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

/// A template as the catalogue reads one: its title pattern, a standalone
/// outcome's side names, and each keyword with its kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    /// The id an outcome names after `template:`, version digits and all.
    pub id: String,
    /// The title, keywords in braces: `{company} IPO confirmed by {dateTime}`.
    pub name: String,
    /// A standalone outcome's two side names, braces allowed.
    pub sides: Vec<String>,
    /// Each keyword and its kind (`string`, `dateTime`, `uDecimal`, `hlPerp`
    /// and so on), in the venue's order.
    pub keywords: Vec<(String, String)>,
}

impl From<&OutcomeTemplate> for Pattern {
    fn from(template: &OutcomeTemplate) -> Pattern {
        Pattern {
            id: template.id.clone(),
            name: template.name.clone(),
            sides: match &template.role {
                TemplateRole::Standalone { side_names } => side_names.clone(),
                _ => Vec::new(),
            },
            keywords: template.keywords.iter().map(|keyword| (keyword.name.clone(), keyword.kind.clone())).collect(),
        }
    }
}

/// The venue's templates by id.
pub type Patterns = BTreeMap<String, Pattern>;

/// The templates as the catalogue reads them.
#[must_use]
pub fn patterns(templates: &[OutcomeTemplate]) -> Patterns {
    templates.iter().map(|template| (template.id.clone(), Pattern::from(template))).collect()
}

/// What a template makes, as `outcomeTemplates` says in its `role`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRole {
    /// `{"standaloneOutcome":{"sideNames"}}`: one outcome and its two sides.
    Standalone { side_names: Vec<String> },
    /// `"question"`.
    Question,
    /// `{"questionOutcome":{"parent"}}`: a named outcome of a question made
    /// from `parent`.
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

/// The `outcomeTemplates` answer. A template that does not read is skipped
/// rather than failing the rest.
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
    Json::Object(vec![("type".to_owned(), Json::String("outcomeTemplates".to_owned()))]).compact()
}

pub async fn outcome_templates(transport: &impl Transport) -> Result<Vec<OutcomeTemplate>, ApiError> {
    info::<Templates>(transport, outcome_templates_request()).await.map(|templates| templates.0)
}

/// `{"type":"outcomeDeployerLimits","venue":..}`.
#[must_use]
pub fn deployer_limits_request(venue: &str) -> String {
    Json::Object(vec![
        ("type".to_owned(), Json::String("outcomeDeployerLimits".to_owned())),
        ("venue".to_owned(), Json::String(venue.to_owned())),
    ])
    .compact()
}

/// What `outcomeDeployerLimits` answers: how many more a venue may register
/// today and how many more it may hold live. Neither cap is documented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Remaining {
    #[serde(default, rename = "nDailyOutcomesRemaining")]
    pub daily: u32,
    #[serde(default, rename = "nActiveOutcomesRemaining")]
    pub active: u32,
}

/// A venue's limits, with the live count beside what remains, so the cap is
/// read from the venue rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeployerLimits {
    /// The venue does not say what cap this counts down from.
    pub daily_remaining: u32,
    pub active_remaining: u32,
    /// Every `outcomeMeta` row on the venue, fallbacks included.
    pub active_used: u32,
}

impl DeployerLimits {
    #[must_use]
    pub const fn new(remaining: Remaining, live: u32) -> DeployerLimits {
        DeployerLimits { daily_remaining: remaining.daily, active_remaining: remaining.active, active_used: live }
    }

    /// Live plus remaining: 100 on every venue so far.
    #[must_use]
    pub const fn active_cap(self) -> u32 {
        self.active_used.saturating_add(self.active_remaining)
    }
}

/// `live` is the venue's count of `outcomeMeta` rows ([`crate::flow::live_on`]).
pub async fn deployer_limits(transport: &impl Transport, venue: &str, live: u32) -> Result<DeployerLimits, ApiError> {
    info::<Remaining>(transport, deployer_limits_request(venue))
        .await
        .map(|remaining| DeployerLimits::new(remaining, live))
}

#[cfg(test)]
#[path = "tests/template.rs"]
mod tests;
