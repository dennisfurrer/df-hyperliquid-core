//! What each constructor refuses, and settlements checked against an `outcomeMeta` read.

use super::*;
use crate::deploy::outcomes::actions::support::{instance, listing, row, venue};
use crate::markets::outcomes::meta::{Question, SideSpec};

#[test]
fn a_settlement_copies_its_names_exactly_as_listed() {
    let meta = listing(vec![row(7, "Chicken  ", " N/A \t", ["Yes ", " No"])]);
    let action = OutcomeDeploy::settle_outcome(venue("ab"), &meta, 7, "0.25".parse().unwrap()).unwrap();
    assert_eq!(
        action.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"settleOutcome":{"outcome":7,"settleFraction":"0.25","details":"","nameAndDescription":["Chicken  "," N/A \t"],"sideNames":["Yes "," No"]}}}"#,
        "trailing spaces and all"
    );
}

/// The venue `out` as live ones are: a standalone outcome, and question 350 of three named
/// outcomes and a fallback, the first named one settled (still listed here, as a read taken
/// mid-settlement can show it).
fn out() -> OutcomeMeta {
    OutcomeMeta {
        questions: vec![Question {
            question: 350,
            name: "template:sportsContestResult".to_owned(),
            description: "participantA:Denmark|participantB:Portugal".to_owned(),
            fallback_outcome: Some(1472),
            named_outcomes: vec![1473, 1474, 1475],
            settled_named_outcomes: vec![1473],
        }],
        ..listing(vec![
            row(
                1209,
                "template:priceTouch",
                "perp:HYPE|target:100|time:20261001-0000",
                ["template:Yes", "template:No"],
            ),
            row(1472, "template fallback", "other", ["Yes", "No"]),
            row(1473, "template:sportsContestParticipant2", "participant:Denmark", ["Yes", "No"]),
            row(1474, "template:sportsContestParticipant2", "participant:Portugal", ["Yes", "No"]),
            row(1475, "template:sportsContestDraw2", "", ["Yes", "No"]),
        ])
    }
}

/// Testnet, 2026-10-08: question 823's named outcomes were settled one at a time at 0 as each
/// team went out, the question staying live.
#[test]
fn a_question_s_leg_settles_alone_only_at_0_or_1_and_never_its_fallback() {
    let meta = out();
    let settle = |outcome, fraction: &str| {
        OutcomeDeploy::settle_outcome(venue("out"), &meta, outcome, fraction.parse().unwrap())
    };
    let leg = settle(1474, "0").unwrap();
    assert!(leg.wire().contains(r#""outcome":1474,"settleFraction":"0""#));
    assert_eq!(settle(1474, "0.5"), Err(DeployError::NotWhole(1474)));
    assert_eq!(settle(1472, "0"), Err(DeployError::Fallback { outcome: 1472, question: 350 }));
    assert_eq!(
        DeployError::Fallback { outcome: 1472, question: 350 }.to_string(),
        "outcome 1472 is question 350's fallback, which settles with its question"
    );
    assert!(settle(1209, "0.25").is_ok(), "an outcome no question holds settles to any fraction");
    assert_eq!(settle(4242, "0"), Err(DeployError::NotListed(4242)));
    let one_sided =
        listing(vec![Outcome { side_specs: vec![SideSpec { name: "Yes".to_owned() }], ..row(9, "x", "", ["", ""]) }]);
    assert_eq!(
        OutcomeDeploy::settle_outcome(venue("out"), &one_sided, 9, Fraction::ONE),
        Err(DeployError::NotTwoSided(9))
    );
}

#[test]
fn a_question_settles_its_open_legs_one_winning_the_settled_one_left_out() {
    let meta = out();
    let settle = |question, winner| OutcomeDeploy::settle_question(venue("out"), &meta, question, winner);
    assert_eq!(
        settle(350, 1475).unwrap().wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleQuestion2":{"question":350,"outcomeSettlements":[{"outcome":1474,"settleFraction":"0","details":"","nameAndDescription":["template:sportsContestParticipant2","participant:Portugal"],"sideNames":["Yes","No"]},{"outcome":1475,"settleFraction":"1","details":"","nameAndDescription":["template:sportsContestDraw2",""],"sideNames":["Yes","No"]}],"nameAndDescription":["template:sportsContestResult","participantA:Denmark|participantB:Portugal"]}}}"#
    );
    assert_eq!(settle(350, 1473), Err(DeployError::NotOpen { outcome: 1473, question: 350 }), "already settled");
    assert_eq!(settle(350, 1472), Err(DeployError::NotOpen { outcome: 1472, question: 350 }), "the fallback");
    assert_eq!(settle(351, 1475), Err(DeployError::NoQuestion(351)));
    let mut gone = out();
    gone.outcomes.retain(|row| row.outcome != 1474);
    assert_eq!(
        OutcomeDeploy::settle_question(venue("out"), &gone, 350, 1475),
        Err(DeployError::NotListed(1474)),
        "a leg the read does not list"
    );
    let mut twice = out();
    twice.questions[0].named_outcomes.push(1474);
    assert_eq!(OutcomeDeploy::settle_question(venue("out"), &twice, 350, 1475), Err(DeployError::SettledTwice(1474)));
}

#[test]
fn a_question_needs_a_named_outcome_and_a_grant_list_a_grant() {
    assert_eq!(
        OutcomeDeploy::register_question(venue("zzz"), instance("sportsContestResult", &[]), FeeScale::ZERO, vec![]),
        Err(DeployError::NoNamedOutcomes)
    );
    assert_eq!(OutcomeDeploy::set_sub_deployers(venue("zzz"), vec![]), Err(DeployError::NoGrants));
}
