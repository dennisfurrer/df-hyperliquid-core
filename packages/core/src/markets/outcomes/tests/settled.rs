//! `settledOutcome` as the venue answered it: outcome 6290 on mainnet, 2026-09-28; outcome 10232 on
//! testnet, 2026-10-08, a leg of question 823; and `null` for testnet's live 23136 the same day.

use super::*;

const MAINNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/settled_outcome_6290_mainnet.json"));
const TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/settled_outcome_10232_testnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_settled_standalone_outcome_names_what_it_paid() {
    let settled: Option<SettledOutcome> = serde_json::from_str(MAINNET).unwrap();
    let settled = settled.unwrap();
    assert_eq!(settled.spec.outcome, 6290);
    assert_eq!(settled.spec.name, "template:binaryPrice");
    assert_eq!(settled.settle_fraction, Dec::ZERO, "read back as 0.0");
    assert_eq!(settled.details, "template");
    assert_eq!(settled.question, None);
    assert_eq!(settled.winner(), Some(OutcomeSide::No));
    assert_eq!((settled.pays(OutcomeSide::Yes), settled.pays(OutcomeSide::No)), (Dec::ZERO, Dec::ONE));
}

#[test]
fn a_settled_leg_names_its_question() {
    let settled: SettledOutcome = serde_json::from_str::<Option<SettledOutcome>>(TESTNET).unwrap().unwrap();
    assert_eq!((settled.spec.outcome, settled.spec.name.as_str()), (10232, "Algeria"));
    assert!(settled.details.starts_with("Algeria was eliminated"));
    let question = settled.question.clone().unwrap();
    assert_eq!((question.question, question.state.as_str()), (823, "active"));
    assert_eq!(question.name, "2026 World Cup champion");
    assert_eq!(settled.winner(), Some(OutcomeSide::No));
}

#[test]
fn an_outcome_not_settled_answers_null() {
    assert_eq!(serde_json::from_str::<Option<SettledOutcome>>("null").unwrap(), None);
}

#[test]
fn a_split_payout_has_no_single_winner() {
    let mut settled: SettledOutcome = serde_json::from_str::<Option<SettledOutcome>>(MAINNET).unwrap().unwrap();
    settled.settle_fraction = dec("0.25");
    assert_eq!(settled.winner(), None);
    assert_eq!((settled.pays(OutcomeSide::Yes), settled.pays(OutcomeSide::No)), (dec("0.25"), dec("0.75")));
    settled.settle_fraction = Dec::ONE;
    assert_eq!(settled.winner(), Some(OutcomeSide::Yes));
}

#[test]
fn the_read_names_the_outcome() {
    assert_eq!(settled_outcome_request(6290), r#"{"type":"settledOutcome","outcome":6290}"#);
}
