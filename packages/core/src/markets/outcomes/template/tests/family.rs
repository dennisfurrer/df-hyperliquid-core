//! Outcome names as the venue writes them, read into a family and a version.

use super::*;

#[test]
fn versions_split_off_the_stem() {
    assert_eq!(Template::classify("template:binaryPrice4"), Template { family: Family::BinaryPrice, version: Some(4) });
    assert_eq!(
        Template::classify("template:sportsContestWinner"),
        Template { family: Family::ContestWinner, version: None }
    );
    assert_eq!(Template::classify("template:policyRateNoChange3").family, Family::PolicyRate(RateMove::NoChange));
}

#[test]
fn names_outside_the_templates_still_classify() {
    assert_eq!(Template::classify("template fallback").family, Family::Fallback);
    assert_eq!(Template::classify("Fallback").family, Family::Fallback, "testnet's spelling");
    assert_eq!(Template::classify("Recurring Fallback").family, Family::Fallback);
    assert_eq!(Template::classify("Recurring Named Outcome").family, Family::Recurring);
    assert_eq!(Template::classify("Argentina").family, Family::Named);
    assert_eq!(
        Template::classify("template:somethingNew").family,
        Family::Template,
        "a template this build has no family for is read from its pattern"
    );
}

#[test]
fn a_question_family_describes_a_question() {
    assert!(Family::ContestResult.is_question() && !Family::ContestWinner.is_question());
    assert!(Family::TournamentWinner.is_question() && Family::PolicyRateDecision.is_question());
}
