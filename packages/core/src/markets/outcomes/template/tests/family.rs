//! Outcome names as the venue writes them, read into a family and a version.

use super::*;

#[test]
fn every_name_reads_into_its_family_and_version() {
    let cases = [
        ("template:binaryPrice4", Family::BinaryPrice, Some(4)),
        ("template:priceTouch", Family::PriceTouch, None),
        ("template:scalePrice2", Family::ScalarPrice, Some(2)),
        ("template:scalarPrice", Family::ScalarPrice, None),
        ("template:sportsContestWinner", Family::ContestWinner, None),
        ("template:sportsContestParticipant", Family::ContestParticipant, None),
        ("template:sportsContestDraw", Family::ContestDraw, None),
        ("template:sportsTournamentParticipant", Family::TournamentParticipant, None),
        ("template:sportsScalarMarket", Family::SportsScalar, None),
        ("template:sportsOverUnderMarket", Family::SportsOverUnder, None),
        ("template:policyRateIncrease", Family::PolicyRate(RateMove::Increase), None),
        ("template:policyRateDecrease", Family::PolicyRate(RateMove::Decrease), None),
        ("template:policyRateNoChange3", Family::PolicyRate(RateMove::NoChange), Some(3)),
        ("template:sportsContestResult", Family::ContestResult, None),
        ("template:sportsTournamentWinner", Family::TournamentWinner, None),
        ("template:policyRateDecision", Family::PolicyRateDecision, None),
        ("template:somethingNew", Family::Template, None),
        ("template fallback", Family::Fallback, None),
        ("Fallback", Family::Fallback, None),
        ("Recurring Fallback", Family::Fallback, None),
        ("Recurring", Family::Recurring, None),
        ("Recurring Named Outcome", Family::Recurring, None),
        ("Argentina", Family::Named, None),
    ];
    for (name, family, version) in cases {
        assert_eq!(Template::classify(name), Template { family, version }, "{name}");
    }
    let questions: Vec<&str> =
        cases.iter().filter(|(_, family, _)| family.is_question()).map(|(name, _, _)| *name).collect();
    assert_eq!(
        questions,
        ["template:sportsContestResult", "template:sportsTournamentWinner", "template:policyRateDecision"],
        "the families that describe a question rather than a book"
    );
}
