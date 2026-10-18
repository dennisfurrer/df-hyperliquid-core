//! `settledOutcome` as the venue answered it: outcome 6290 on mainnet, 2026-09-28, and outcome
//! 10232 on testnet, 2026-10-08, a leg of question 823. Reads through a client are in
//! `tests/mocked/markets`.

use super::*;

const MAINNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/settled_outcome_6290_mainnet.json"));
const TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/settled_outcome_10232_testnet.json"));

fn settled(text: &str) -> SettledOutcome {
    serde_json::from_str::<Option<SettledOutcome>>(text).unwrap().unwrap()
}

#[test]
fn a_settled_outcome_names_what_each_side_was_paid() {
    let standalone = settled(MAINNET);
    assert_eq!((standalone.spec.outcome, standalone.spec.name.as_str()), (6290, "template:binaryPrice"));
    assert_eq!((standalone.settle_fraction, standalone.details.as_str()), (Dec::ZERO, "template"), "read back as 0.0");
    assert_eq!(standalone.question, None);
    let cases = [
        ("0", Some(OutcomeSide::No), "0", "1"),
        ("1", Some(OutcomeSide::Yes), "1", "0"),
        ("0.25", None, "0.25", "0.75"),
    ];
    for (fraction, winner, yes, no) in cases {
        let paid = SettledOutcome { settle_fraction: Dec::parse(fraction).unwrap(), ..standalone.clone() };
        let pays = (paid.pays(OutcomeSide::Yes), paid.pays(OutcomeSide::No));
        assert_eq!((paid.winner(), pays), (winner, (Dec::parse(yes).unwrap(), Dec::parse(no).unwrap())), "{fraction}");
    }
    assert_eq!(serde_json::from_str::<Option<SettledOutcome>>("null").unwrap(), None, "not settled yet");
}

#[test]
fn a_settled_leg_names_its_question_and_its_state() {
    let leg = settled(TESTNET);
    assert_eq!((leg.spec.outcome, leg.spec.name.as_str(), leg.winner()), (10232, "Algeria", Some(OutcomeSide::No)));
    assert!(leg.details.starts_with("Algeria was eliminated"));
    let question = leg.question.unwrap();
    assert_eq!((question.question, question.state.as_str()), (823, "active"));
    assert_eq!(question.name, "2026 World Cup champion");
    let nameless = r#"{"spec":{"outcome":1},"settleFraction":"1","question":{"question":{},"name":"q"}}"#;
    assert!(serde_json::from_str::<SettledOutcome>(nameless).is_err(), "a leg's question must name its id");
}
