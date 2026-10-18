//! An outcome's name read: `template:<id><version>` gives the [`Family`] of market it is and the
//! template's version; a fallback, a `Recurring` name and a deployer's own name read too.

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

#[cfg(test)]
#[path = "tests/family.rs"]
mod tests;
