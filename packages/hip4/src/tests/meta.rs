use hl_core::L1Action;

use super::*;
use crate::deploy::OutcomeDeploy;

/// Live, mainnet, 2026-10-06, trimmed: a question's fallback and one of its
/// named outcomes, a recurring outcome with no venue, the question, and one
/// venue's deployer.
const META: &str = r#"{"outcomes":[{"outcome":1472,"name":"template fallback","description":"other","sideSpecs":[{"name":"Yes"},{"name":"No"}],"quoteToken":"USDC","venue":"out","deployerFeeScale":"1.0"},{"outcome":1473,"name":"template:sportsTournamentParticipant","description":"participant:Arsenal","sideSpecs":[{"name":"Yes"},{"name":"No"}],"quoteToken":"USDC","venue":"out","deployerFeeScale":"1.0"},{"outcome":9015,"name":"Recurring","description":"class:priceBinary|underlying:BTC|expiry:20261007-0600|targetPrice:85501|period:1d","sideSpecs":[{"name":"Yes"},{"name":"No"}],"quoteToken":"USDC"}],"questions":[{"question":198,"name":"template:sportsTournamentWinner","description":"competition:English Premier League|officialSource:English Premier League|resolutionDeadline:20270605-1200|season:2026/2027|sport:football/soccer","fallbackOutcome":1472,"namedOutcomes":[1473,1474,1475,1476,1477,1478],"settledNamedOutcomes":[]}],"deployers":[{"deployer":"0x08e9c89f46dccee91bdb85c6532eb93a4c335efe","venue":"skew","subDeployers":[["registerAndAssociateNamedOutcomeFromTemplate",["0x1c867861e0cffb0eba07d9d94f716189c130dac6"]],["registerQuestionFromTemplate",["0x1c867861e0cffb0eba07d9d94f716189c130dac6"]],["registerStandaloneOutcomeFromTemplate",["0x1c867861e0cffb0eba07d9d94f716189c130dac6"]],["settleOutcome",["0x1c867861e0cffb0eba07d9d94f716189c130dac6"]],["settleQuestion",["0x1c867861e0cffb0eba07d9d94f716189c130dac6"]]]}],"feeScale":"1.0"}"#;

fn meta() -> OutcomeMeta {
    serde_json::from_str(META).unwrap()
}

#[test]
fn outcome_meta_reads_the_live_answer() {
    let meta = meta();
    assert_eq!((meta.outcomes.len(), meta.fee_scale), (3, Some(Dec::ONE)));
    let arsenal = meta.outcome(1473).unwrap();
    assert_eq!(arsenal.description, "participant:Arsenal");
    assert_eq!(arsenal.deployer_fee_scale, Some(Dec::ONE), "read back as 1.0");
    assert_eq!(arsenal.coin(OutcomeSide::No).to_string(), "#14731");
    let recurring = meta.outcome(9015).unwrap();
    assert_eq!((recurring.venue.as_deref(), recurring.quote_token.as_deref()), (None, Some("USDC")));
    let question = meta.question_of(1473).unwrap();
    assert_eq!(question.question, 198);
    assert_eq!(meta.question_of(1472).map(|question| question.question), Some(198), "the fallback too");
    assert_eq!(question.unsettled().count(), 6);
    assert_eq!(meta.question_of(9015), None);
    let skew = meta.deployer("skew").unwrap();
    assert_eq!(skew.deployer.to_hex(), "0x08e9c89f46dccee91bdb85c6532eb93a4c335efe");
    assert_eq!(skew.sub_deployers.len(), 5);
    assert_eq!(serde_json::from_str::<OutcomeMeta>("{}").unwrap(), OutcomeMeta::default());
}

/// A settlement repeats the outcome's name, description and side names as
/// listed; [`Outcome::settlement`] copies them rather than having them typed.
#[test]
fn a_settlement_copies_what_outcome_meta_lists() {
    let meta = meta();
    let settlement = meta.outcome(9015).unwrap().settlement(Fraction::ONE).unwrap();
    let action = OutcomeDeploy::settle_outcome("out".parse().unwrap(), settlement);
    assert!(action.operation.is_settlement());
    assert_eq!(
        action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleOutcome":{"outcome":9015,"settleFraction":"1","details":"","nameAndDescription":["Recurring","class:priceBinary|underlying:BTC|expiry:20261007-0600|targetPrice:85501|period:1d"],"sideNames":["Yes","No"]}}}"#
    );
    let one_sided =
        Outcome { side_specs: vec![SideSpec { name: "Yes".to_owned() }], ..meta.outcome(9015).unwrap().clone() };
    assert_eq!(one_sided.settlement(Fraction::ONE), None);
}

#[test]
fn the_read_names_its_type() {
    assert_eq!(outcome_meta_request(), r#"{"type":"outcomeMeta"}"#);
}

/// hip4-tui's `core/src/tests/chain.rs`: a live delegation table, one
/// address in upper case as the venue sometimes writes it.
#[test]
fn the_delegation_table_is_read_by_key_and_not_by_action() {
    let text = r#"{"outcomes": [], "deployers": [{
        "deployer": "0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e",
        "venue": "out",
        "subDeployers": [
          ["registerQuestionFromTemplate",
           ["0x6947A610EF50F8B5D4B59ABCD94DD75AAAA645B9", "0xf1923927d7d2847191fb7ef8b1a16028aa5ae754"]],
          ["settleOutcome",
           ["0x6947a610ef50f8b5d4b59abcd94dd75aaaa645b9", "0xf1923927d7d2847191fb7ef8b1a16028aa5ae754"]],
          ["settleQuestion", ["0xf1923927d7d2847191fb7ef8b1a16028aa5ae754"]]
        ]}]}"#;
    let meta: OutcomeMeta = serde_json::from_str(text).unwrap();
    let keys = meta.deployers[0].keys();
    assert_eq!(keys.len(), 2, "five grants across two addresses");
    assert_eq!(keys[0].address.to_hex(), "0xf1923927d7d2847191fb7ef8b1a16028aa5ae754");
    assert_eq!(keys[0].actions, ["registerQuestionFromTemplate", "settleOutcome", "settleQuestion"]);
    assert_eq!(keys[1].address.to_hex(), "0x6947a610ef50f8b5d4b59abcd94dd75aaaa645b9");
    assert!(keys[0].settles() && keys[0].registers());
    assert_eq!(meta.venue_of("out"), Some("out"));
    assert_eq!(meta.venue_of("OUT"), Some("out"), "a venue's name, case aside");
    assert_eq!(meta.venue_of("0x0C46EB73FAE2816F219FCF11F50D6D3C59B5819E"), Some("out"), "or its deployer");
    assert_eq!(meta.venue_of("0x0000000000000000000000000000000000000000"), None);
    assert_eq!(meta.venue_of("skew"), None);
}
