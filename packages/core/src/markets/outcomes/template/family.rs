//! An outcome's name read: `template:<id><version>` gives the [`Family`] of market it is and the
//! template's version; a fallback, a `Recurring` name and a deployer's own name read too.

/// Which way a policy rate moves, for the three named outcomes of a rate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateMove {
    /// `increase`: the rate goes up.
    Increase,
    /// `decrease`: the rate goes down.
    Decrease,
    /// `no_change`: the rate stays.
    NoChange,
}

/// What kind of market a template makes, read from its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// `binaryPrice`: whether a perp's price is above a threshold at a time.
    BinaryPrice,
    /// `priceTouch`: whether a perp's price touches a target at or before a time.
    PriceTouch,
    /// Read from the template id `scalarPrice`, or `scalePrice`.
    ScalarPrice,
    /// `sportsContestWinner`: a contest between two participants, one side for each.
    ContestWinner,
    /// `sportsContestParticipant`: one participant's named outcome in a contest result question.
    ContestParticipant,
    /// `sportsContestDraw`: the draw named outcome in a contest result question.
    ContestDraw,
    /// `sportsTournamentParticipant`: one participant's named outcome in a tournament winner question.
    TournamentParticipant,
    /// Read from the template id `sportsScalarMarket`.
    SportsScalar,
    /// Read from the template id `sportsOverUnderMarket`.
    SportsOverUnder,
    /// `policyRateIncrease`, `policyRateDecrease` or `policyRateNoChange`: one move of a policy rate
    /// decision.
    PolicyRate(RateMove),
    /// The venue's own daily markets, named `Recurring`.
    Recurring,
    /// A question's fallback outcome: `template fallback`, testnet's `Fallback`, or `Recurring Fallback`.
    Fallback,
    /// A deployer's own name, no template.
    Named,
    /// `sportsContestResult`: a question with one named outcome per participant and one for a draw.
    ContestResult,
    /// `sportsTournamentWinner`: a question with one named outcome per participant in a tournament.
    TournamentWinner,
    /// `policyRateDecision`: a question with one named outcome per rate move.
    PolicyRateDecision,
    /// A venue template with no family here; its `outcomeTemplates` entry says what it makes.
    Template,
}

impl Family {
    /// Every family, in the order a listing reports them.
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

    /// Its name in snake case, as serde writes it and a filter names it: `binary_price`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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

impl std::fmt::Display for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A name [`Family::as_str`] does not write.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown kind `{0}`")]
pub struct UnknownFamily(pub String);

impl std::str::FromStr for Family {
    type Err = UnknownFamily;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wanted = text.trim();
        Family::ALL.into_iter().find(|family| family.as_str() == wanted).ok_or_else(|| UnknownFamily(wanted.to_owned()))
    }
}

impl serde::Serialize for Family {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Family {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

/// An outcome's name read: its family and the version digits after the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    /// What kind of market the name says it is.
    pub family: Family,
    /// The digits after the template id; `None` without them, or past 255.
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

#[cfg(test)]
#[path = "tests/family.rs"]
mod tests;
