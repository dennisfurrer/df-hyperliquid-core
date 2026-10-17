//! Deployer vectors made outside this crate:
//!
//! - Hyperliquid's Python SDK (`hyperliquid.utils.signing.action_hash`) at nonce 1755530000000, no vault:
//!   every outcome and activate L1 hash, and the three inner multi-sig hashes over an outcome action.
//! - An independent builder's msgpack, L1 hash and mainnet digest at nonce 1234567890000.
//! - Preset bodies the venue takes.
//!
//! A settlement copies `outcomeMeta`, so each settle below reads one listing what it settles.

use crate::client::Network;
use crate::markets::outcomes::meta::{OutcomeMeta, Question, SideSpec};
use crate::markets::outcomes::template::Templates;
use crate::utils::sign::{L1Action, Value, connection_id};
use crate::utils::{Address, hex};

use super::*;

const TESTNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));
const MAINNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));

const MASTER: &str = "0xa8d106881abe9a0b6ffd367788d6af91439d37f6";
const LEADER: &str = "0x251fd19df63a83652eea93a6a5e451aa5ce81e4c";
const AGENT: &str = "0xab04c8d8372d83ec939570fd2d956d0bb31aace3";
const NONCE: u64 = 1_755_530_000_000;
const GOLDEN_NONCE: u64 = 1_234_567_890_000;

/// 2026-09-30 12:00 UTC, the clock every dated check here reads.
const NOW_MS: u64 = 1_790_769_600_000;

fn venue(name: &str) -> Venue {
    name.parse().unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn keywords(pairs: &[(&str, &str)]) -> Keywords {
    Keywords::from_pairs(pairs.iter().copied()).unwrap()
}

fn instance(id: &str, pairs: &[(&str, &str)]) -> Instance {
    Instance::new(id, keywords(pairs))
}

/// An outcome as `outcomeMeta` lists it, its words exactly as given.
fn row(outcome: u32, name: &str, description: &str, sides: [&str; 2]) -> Outcome {
    Outcome {
        outcome,
        name: name.to_owned(),
        description: description.to_owned(),
        side_specs: sides.iter().map(|side| SideSpec { name: (*side).to_owned() }).collect(),
        quote_token: Some("USDC".to_owned()),
        venue: None,
        deployer_fee_scale: None,
    }
}

/// `outcomeMeta` listing `outcomes`.
fn listing(outcomes: Vec<Outcome>) -> OutcomeMeta {
    OutcomeMeta { outcomes, ..OutcomeMeta::default() }
}

/// `outcomeMeta` listing `legs` as the named outcomes of `question`, none settled, no fallback.
fn asked(question: u32, name: &str, description: &str, legs: Vec<Outcome>) -> OutcomeMeta {
    let named_outcomes = legs.iter().map(|leg| leg.outcome).collect();
    OutcomeMeta {
        questions: vec![Question {
            question,
            name: name.to_owned(),
            description: description.to_owned(),
            fallback_outcome: None,
            named_outcomes,
            settled_named_outcomes: Vec::new(),
        }],
        ..listing(legs)
    }
}

/// The Python script's actions, built the way a caller builds them: the
/// keywords given out of order, the fee scale typed with a trailing zero.
fn python_standalone() -> OutcomeDeploy {
    OutcomeDeploy::register_standalone(
        venue("zzz"),
        instance(
            "binaryPrice4",
            &[
                ("time", "20260901-1200"),
                ("threshold", "50000"),
                ("perp", "BTC"),
                ("seconds", "60"),
                ("priceDescription", "Bitcoin"),
            ],
        ),
        FeeScale::ZERO,
    )
}

fn python_question() -> OutcomeDeploy {
    OutcomeDeploy::register_question(
        venue("zzz"),
        instance("sportsContestResult", &[("participantB", "Spain"), ("participantA", "Brazil")]),
        "1.50".parse().unwrap(),
        vec![
            instance("sportsContestParticipant", &[("participant", "Brazil")]),
            Instance::new("sportsContestDraw", Keywords::new()),
        ],
    )
    .unwrap()
}

fn python_associate() -> OutcomeDeploy {
    OutcomeDeploy::register_and_associate(
        venue("zzz"),
        182,
        instance("sportsContestParticipant", &[("participant", "Spain")]),
    )
}

fn python_sub_deployers() -> OutcomeDeploy {
    OutcomeDeploy::set_sub_deployers(
        venue("zzz"),
        vec![
            Grant { permission: Permission::SettleOutcome, user: address(AGENT), allowed: true },
            Grant { permission: Permission::SettleQuestion, user: address(LEADER), allowed: false },
        ],
    )
    .unwrap()
}

fn python_settle_outcome() -> OutcomeDeploy {
    let meta = listing(vec![row(
        13065,
        "template:binaryPrice4",
        "perp:BTC|threshold:50000|time:20260901-1200",
        ["template:Yes", "template:No"],
    )]);
    OutcomeDeploy::settle_outcome(venue("zzz"), &meta, 13065, Fraction::ONE).unwrap()
}

/// Question 182 of three chickens, 7004 winning.
fn chickens() -> OutcomeMeta {
    let chicken = |outcome| row(outcome, "Chicken", "N/A", ["Yes", "No"]);
    asked(182, "What will Hypurr eat most of?", "A food journal", vec![chicken(7003), chicken(7004), chicken(7005)])
}

fn python_settle_question() -> OutcomeDeploy {
    OutcomeDeploy::settle_question(venue("zzz"), &chickens(), 182, 7004).unwrap()
}

fn python_activate() -> ActivateOutcomeDeployer {
    ActivateOutcomeDeployer::Activate(venue("zzz"))
}

#[test]
fn every_outcome_action_hashes_as_the_python_sdk_hashes_it() {
    let cases: [(&str, &dyn L1Action, &str, &str); 8] = [
        (
            "registerStandaloneOutcome",
            &python_standalone(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"registerStandaloneOutcomeFromTemplate":{"id":"binaryPrice4","keywordToValue":[["perp","BTC"],["priceDescription","Bitcoin"],["seconds","60"],["threshold","50000"],["time","20260901-1200"]],"deployerFeeScale":"0"}}}"#,
            "0xe76ca1311e7a15df0ad87a5dab3a02b047d23837ff001d480d5f849fded4a9c6",
        ),
        (
            "registerQuestion",
            &python_question(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"registerQuestionFromTemplate":{"questionTemplateInstance":{"id":"sportsContestResult","keywordToValue":[["participantA","Brazil"],["participantB","Spain"]],"deployerFeeScale":"1.5"},"namedOutcomeTemplateInstances":[{"id":"sportsContestParticipant","keywordToValue":[["participant","Brazil"]]},{"id":"sportsContestDraw","keywordToValue":[]}]}}}"#,
            "0x3d1f7e8e0409bf566bdf0201478aa6fe8293ed4dbb9d30b07c1b32230aaa99d5",
        ),
        (
            "registerAndAssociateNamedOutcome",
            &python_associate(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"registerAndAssociateNamedOutcomeFromTemplate":{"question":182,"namedOutcomeTemplateInstance":{"id":"sportsContestParticipant","keywordToValue":[["participant","Spain"]]}}}}"#,
            "0xda72d2939a491585c4781553fa6881e045207edbe320ee31c58e9e767768dce6",
        ),
        (
            "setSubDeployers",
            &python_sub_deployers(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"setSubDeployers":[{"variant":"settleOutcome","user":"0xab04c8d8372d83ec939570fd2d956d0bb31aace3","allowed":true},{"variant":"settleQuestion","user":"0x251fd19df63a83652eea93a6a5e451aa5ce81e4c","allowed":false}]}}"#,
            "0x13e98e1ec9d18a7374ffd6efd908d9b8597bb702d8fabe4c320569b96a5993fa",
        ),
        (
            "settleOutcome",
            &python_settle_outcome(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"settleOutcome":{"outcome":13065,"settleFraction":"1","details":"","nameAndDescription":["template:binaryPrice4","perp:BTC|threshold:50000|time:20260901-1200"],"sideNames":["template:Yes","template:No"]}}}"#,
            "0x46bc607e52ce7d6bbb25329f33fc9416046efc06aecad3cb8c03b11491ab7bb6",
        ),
        (
            "settleQuestion",
            &python_settle_question(),
            r#"{"type":"outcomeDeploy","venue":"zzz","operation":{"settleQuestion2":{"question":182,"outcomeSettlements":[{"outcome":7003,"settleFraction":"0","details":"","nameAndDescription":["Chicken","N/A"],"sideNames":["Yes","No"]},{"outcome":7004,"settleFraction":"1","details":"","nameAndDescription":["Chicken","N/A"],"sideNames":["Yes","No"]},{"outcome":7005,"settleFraction":"0","details":"","nameAndDescription":["Chicken","N/A"],"sideNames":["Yes","No"]}],"nameAndDescription":["What will Hypurr eat most of?","A food journal"]}}}"#,
            "0x8bf875b43394c89e151367f995a9735cc0073c4c39c65724cb11cf9f3123c655",
        ),
        (
            "activate",
            &python_activate(),
            r#"{"type":"activateOutcomeDeployer","activate":{"venueName":"zzz"}}"#,
            "0xb6527b4ba1d06d7884f85a2f0c165fde59becd3bc7f8725bc0f481e3a2c7f1cd",
        ),
        (
            "deactivate",
            &ActivateOutcomeDeployer::Deactivate,
            r#"{"type":"activateOutcomeDeployer","deactivate":null}"#,
            "0xe91d6e3ccf382a7ae37e78d8e6dadf2f168cbedee74c0f2b984b2b8561bc008f",
        ),
    ];
    for (name, action, body, id) in cases {
        assert_eq!(action.wire(), body, "{name}: the body");
        assert_eq!(hex::encode_prefixed(&action.connection_id(NONCE, None)), id, "{name}: the L1 hash");
    }
}

/// What a multi-sig signer signs over is `[multiSigUser, outerSigner, action]`
/// packed and hashed as an L1 action; built by hand here only to check the
/// outcome action nests byte for byte.
#[test]
fn an_outcome_action_nests_in_a_multi_sig_as_the_python_sdk_nests_it() {
    let inner = |action: Map| {
        let packed = Value::Array(vec![Value::from(MASTER), Value::from(LEADER), Value::Map(action)]).to_msgpack();
        hex::encode_prefixed(&connection_id(&packed, NONCE, None))
    };
    assert_eq!(
        inner(python_standalone().action()),
        "0xaae5cb534172068ed0db7195034da985961d06bb5af93c984a98341b80bf3f31",
        "registerStandaloneOutcome"
    );
    assert_eq!(
        inner(python_settle_question().action()),
        "0x396ad68ec562d8908fe49611b660449263c64ff7a0b79d01a8e07221220e65e9",
        "settleQuestion"
    );
    assert_eq!(
        inner(python_activate().action()),
        "0x32e4f02e2a5749f37081f1990e55fa5b576ae623adaecc89a5bc6edd453f3203",
        "activate"
    );
}

/// `(msgpack, L1 hash, mainnet digest)` as the independent builder pins them.
fn assert_golden(name: &str, action: &impl L1Action, msgpack: &str, id: &str, digest: &str) {
    assert_eq!(hex::encode(&action.msgpack()), msgpack, "{name}: the msgpack");
    assert_eq!(hex::encode(&action.connection_id(GOLDEN_NONCE, None)), id, "{name}: the L1 hash");
    assert_eq!(hex::encode(&action.digest(GOLDEN_NONCE, None, Network::Mainnet)), digest, "{name}: the digest");
}

#[test]
fn outcome_actions_pack_byte_for_byte_as_a_production_builder_packs_them() {
    let standalone = OutcomeDeploy::register_standalone(
        venue("out"),
        instance(
            "binaryPrice",
            &[
                ("perp", "BTC"),
                ("priceDescription", "BTC-USDC mark"),
                ("seconds", "1"),
                ("threshold", "100000"),
                ("time", "20261001-0000"),
            ],
        ),
        FeeScale::ONE,
    );
    assert_golden(
        "standalone",
        &standalone,
        "83a474797065ad6f7574636f6d654465706c6f79a576656e7565a36f7574a96f7065726174696f6e81d92572656769737465725374616e64616c6f6e654f7574636f6d6546726f6d54656d706c61746583a26964ab62696e6172795072696365ae6b6579776f7264546f56616c75659592a470657270a342544392b070726963654465736372697074696f6ead4254432d55534443206d61726b92a77365636f6e6473a13192a97468726573686f6c64a631303030303092a474696d65ad32303236313030312d30303030b06465706c6f7965724665655363616c65a131",
        "daa18de77c967c757dc4e833258e80e0dce198216c41eba4743f43d9b6a6fe46",
        "254e054cfcfdf83a54c5e42b2ac8f46c85503660c0ad3b69576eea7443a3a964",
    );

    let gold = listing(vec![row(
        1224,
        "template:binaryPrice",
        "perp:GOLD|priceDescription:d|seconds:1|threshold:4460.9|time:20260829-2100",
        ["template:Yes", "template:No"],
    )]);
    let settle = OutcomeDeploy::settle_outcome(venue("out"), &gold, 1224, Fraction::ONE).unwrap();
    assert_golden(
        "settleOutcome",
        &settle,
        "83a474797065ad6f7574636f6d654465706c6f79a576656e7565a36f7574a96f7065726174696f6e81ad736574746c654f7574636f6d6585a76f7574636f6d65cd04c8ae736574746c654672616374696f6ea131a764657461696c73a0b26e616d65416e644465736372697074696f6e92b474656d706c6174653a62696e6172795072696365d94a706572703a474f4c447c70726963654465736372697074696f6e3a647c7365636f6e64733a317c7468726573686f6c643a343436302e397c74696d653a32303236303832392d32313030a9736964654e616d657392ac74656d706c6174653a596573ab74656d706c6174653a4e6f",
        "aa99e61e2a36a199f7c41e02adaeceb87a33125c13cc21736135334ca354f84b",
        "ea144f400000095bb6bc2b95822c8f3e888793322e6382b707b82b113243b96a",
    );

    let fomc = "decisionLabel:September 2026|institution:FOMC";
    let decision = asked(
        191,
        "template:policyRateDecision",
        fomc,
        vec![
            row(1227, "template:policyRateHike", fomc, ["Yes", "No"]),
            row(1228, "template:policyRateNoChange", fomc, ["Yes", "No"]),
        ],
    );
    let question = OutcomeDeploy::settle_question(venue("out"), &decision, 191, 1228).unwrap();
    assert_golden(
        "settleQuestion2",
        &question,
        "83a474797065ad6f7574636f6d654465706c6f79a576656e7565a36f7574a96f7065726174696f6e81af736574746c655175657374696f6e3283a87175657374696f6eccbfb26f7574636f6d65536574746c656d656e74739285a76f7574636f6d65cd04cbae736574746c654672616374696f6ea130a764657461696c73a0b26e616d65416e644465736372697074696f6e92b774656d706c6174653a706f6c6963795261746548696b65d92d6465636973696f6e4c6162656c3a53657074656d62657220323032367c696e737469747574696f6e3a464f4d43a9736964654e616d657392a3596573a24e6f85a76f7574636f6d65cd04ccae736574746c654672616374696f6ea131a764657461696c73a0b26e616d65416e644465736372697074696f6e92bb74656d706c6174653a706f6c696379526174654e6f4368616e6765d92d6465636973696f6e4c6162656c3a53657074656d62657220323032367c696e737469747574696f6e3a464f4d43a9736964654e616d657392a3596573a24e6fb26e616d65416e644465736372697074696f6e92bb74656d706c6174653a706f6c696379526174654465636973696f6ed92d6465636973696f6e4c6162656c3a53657074656d62657220323032367c696e737469747574696f6e3a464f4d43",
        "f0d9b5453e8e4c276a4bdfa02031090e67a667ffb3907d10988e5c529547727e",
        "92da1b308b73beaa73333d333f4a5a2afb5f4a9f709f5a5230347e2a4ca4805b",
    );
}

const CONTEST: &str = "competition:UEFA Nations League|contestType:Match|countedPlay:regulation time, 90 minutes plus stoppage time|officialSource:Union of European Football Associations|participantA:Denmark|participantB:Portugal|resolutionDeadline:20261002-1845|scheduledStart:20261001-1845|season:2026/27|sport:Soccer|stage:League A, Matchday 3";

#[test]
fn every_builder_writes_the_recorded_presets_byte_for_byte() {
    let ab = || venue("ab");
    let standalone = OutcomeDeploy::register_standalone(
        ab(),
        instance(
            "binaryPrice",
            &[
                ("threshold", "83114"),
                ("time", "20261231-1200"),
                ("seconds", "60"),
                ("priceDescription", "BTC-USDC perp mark"),
                ("perp", "BTC"),
            ],
        ),
        "1.0".parse().unwrap(),
    );
    assert_eq!(
        standalone.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"registerStandaloneOutcomeFromTemplate":{"id":"binaryPrice","keywordToValue":[["perp","BTC"],["priceDescription","BTC-USDC perp mark"],["seconds","60"],["threshold","83114"],["time","20261231-1200"]],"deployerFeeScale":"1"}}}"#
    );

    let question = OutcomeDeploy::register_question(
        ab(),
        instance(
            "sportsContestResult",
            &[
                ("stage", "League A, Matchday 3"),
                ("sport", "Soccer"),
                ("season", "2026/27"),
                ("scheduledStart", "20261001-1845"),
                ("resolutionDeadline", "20261002-1845"),
                ("participantB", "Portugal"),
                ("participantA", "Denmark"),
                ("officialSource", "Union of European Football Associations"),
                ("countedPlay", "regulation time, 90 minutes plus stoppage time"),
                ("contestType", "Match"),
                ("competition", "UEFA Nations League"),
            ],
        ),
        FeeScale::ONE,
        vec![
            instance("sportsContestParticipant2", &[("participant", "Denmark")]),
            instance("sportsContestDraw2", &[]),
            instance("sportsContestParticipant2", &[("participant", "Portugal")]),
        ],
    )
    .unwrap();
    assert_eq!(
        question.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"registerQuestionFromTemplate":{"questionTemplateInstance":{"id":"sportsContestResult","keywordToValue":[["competition","UEFA Nations League"],["contestType","Match"],["countedPlay","regulation time, 90 minutes plus stoppage time"],["officialSource","Union of European Football Associations"],["participantA","Denmark"],["participantB","Portugal"],["resolutionDeadline","20261002-1845"],["scheduledStart","20261001-1845"],["season","2026/27"],["sport","Soccer"],["stage","League A, Matchday 3"]],"deployerFeeScale":"1"},"namedOutcomeTemplateInstances":[{"id":"sportsContestParticipant2","keywordToValue":[["participant","Denmark"]]},{"id":"sportsContestDraw2","keywordToValue":[]},{"id":"sportsContestParticipant2","keywordToValue":[["participant","Portugal"]]}]}}}"#,
        "named outcomes keep the order given, keywords do not"
    );

    let associate = OutcomeDeploy::register_and_associate(
        ab(),
        350,
        instance("sportsContestParticipant2", &[("participant", "Spain")]),
    );
    assert_eq!(
        associate.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"registerAndAssociateNamedOutcomeFromTemplate":{"question":350,"namedOutcomeTemplateInstance":{"id":"sportsContestParticipant2","keywordToValue":[["participant","Spain"]]}}}}"#
    );

    let btc = listing(vec![row(
        6955,
        "template:binaryPrice",
        "perp:BTC|priceDescription:BTC-USDC perp mark|seconds:60|threshold:83114|time:20260930-0845",
        ["template:Yes", "template:No"],
    )]);
    let settle = OutcomeDeploy::settle_outcome(ab(), &btc, 6955, Fraction::ONE).unwrap();
    assert_eq!(
        settle.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"settleOutcome":{"outcome":6955,"settleFraction":"1","details":"","nameAndDescription":["template:binaryPrice","perp:BTC|priceDescription:BTC-USDC perp mark|seconds:60|threshold:83114|time:20260930-0845"],"sideNames":["template:Yes","template:No"]}}}"#
    );

    let named = "template:sportsContestParticipant2";
    let contest = asked(
        350,
        "template:sportsContestResult",
        CONTEST,
        vec![
            row(5940, named, "participant:Denmark", ["Yes", "No"]),
            row(5941, "template:sportsContestDraw2", "", ["Yes", "No"]),
            row(5942, named, "participant:Portugal", ["Yes", "No"]),
        ],
    );
    let settle_question = OutcomeDeploy::settle_question(ab(), &contest, 350, 5940).unwrap();
    assert_eq!(
        settle_question.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"settleQuestion2":{"question":350,"outcomeSettlements":[{"outcome":5940,"settleFraction":"1","details":"","nameAndDescription":["template:sportsContestParticipant2","participant:Denmark"],"sideNames":["Yes","No"]},{"outcome":5941,"settleFraction":"0","details":"","nameAndDescription":["template:sportsContestDraw2",""],"sideNames":["Yes","No"]},{"outcome":5942,"settleFraction":"0","details":"","nameAndDescription":["template:sportsContestParticipant2","participant:Portugal"],"sideNames":["Yes","No"]}],"nameAndDescription":["template:sportsContestResult","competition:UEFA Nations League|contestType:Match|countedPlay:regulation time, 90 minutes plus stoppage time|officialSource:Union of European Football Associations|participantA:Denmark|participantB:Portugal|resolutionDeadline:20261002-1845|scheduledStart:20261001-1845|season:2026/27|sport:Soccer|stage:League A, Matchday 3"]}}}"#
    );

    let grants = OutcomeDeploy::set_sub_deployers(
        ab(),
        vec![Grant { permission: Permission::SettleOutcome, user: Address::ZERO, allowed: true }],
    )
    .unwrap();
    assert_eq!(
        grants.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"setSubDeployers":[{"variant":"settleOutcome","user":"0x0000000000000000000000000000000000000000","allowed":true}]}}"#
    );

    assert_eq!(
        ActivateOutcomeDeployer::Activate(ab()).wire(),
        r#"{"type":"activateOutcomeDeployer","activate":{"venueName":"ab"}}"#
    );
    assert_eq!(
        ActivateOutcomeDeployer::Deactivate.wire(),
        r#"{"type":"activateOutcomeDeployer","deactivate":null}"#,
        "the key is sent, with nil"
    );
    assert_eq!(
        hex::encode(&ActivateOutcomeDeployer::Deactivate.msgpack()),
        "82a474797065b761637469766174654f7574636f6d654465706c6f796572aa64656163746976617465c0",
        "and packs as msgpack nil, the last byte"
    );
}

#[test]
fn keywords_come_out_sorted_bytewise_however_they_go_in() {
    let pairs = [
        ("time", "1"),
        ("zeta", "2"),
        ("\u{e9}clair", "3"),
        ("participantB", "4"),
        ("Zone", "5"),
        ("participantA", "6"),
        ("_x", "7"),
        ("apple", "8"),
        ("Apple", "9"),
    ];
    let sorted = concat!(
        r#"[["Apple","9"],["Zone","5"],["_x","7"],["apple","8"],["participantA","6"],"#,
        r#"["participantB","4"],["time","1"],["zeta","2"],[""#,
        "\u{e9}",
        r#"clair","3"]]"#,
    );
    let forward = keywords(&pairs);
    let mut reversed = pairs;
    reversed.reverse();
    let backward = keywords(&reversed);
    for built in [&forward, &backward] {
        assert_eq!(built.wire().to_json().compact(), sorted, "upper case before lower case, ASCII before the rest");
    }
    assert_eq!(forward, backward);
    assert_eq!(forward.len(), 9);
    assert_eq!(forward.get("Zone"), Some("5"));
    assert_eq!(forward.get("zone"), None);
    assert_eq!(Keywords::new().wire().to_json().compact(), "[]", "a template with no keywords");
    assert!(Keywords::new().is_empty());
}

#[test]
fn a_keyword_takes_one_plain_value() {
    let refused = |pairs: &[(&str, &str)]| Keywords::from_pairs(pairs.iter().copied()).unwrap_err().to_string();
    assert_eq!(refused(&[("participant", "Brazil"), ("participant", "Spain")]), "participant is given twice");
    assert_eq!(refused(&[("perp", "")]), "perp needs a value");
    assert_eq!(refused(&[("perp", "BTC|ETH")]), "perp cannot hold {, } or |");
    assert_eq!(refused(&[("perp", "{perp}")]), "perp cannot hold {, } or |");
    let too_long = "x".repeat(101);
    let too_many_bytes = "\u{e9}".repeat(51);
    assert_eq!(refused(&[("priceDescription", too_long.as_str())]), "priceDescription takes at most 100 characters");
    assert_eq!(
        refused(&[("priceDescription", too_many_bytes.as_str())]),
        "priceDescription takes at most 100 characters",
        "counted in bytes: 51 of them are 102"
    );
    let longest = "x".repeat(100);
    assert_eq!(keywords(&[("priceDescription", longest.as_str())]).get("priceDescription"), Some(longest.as_str()));
}

#[test]
fn a_hint_is_read_by_its_name_and_an_unknown_one_is_kept() {
    for name in ["dateTime", "date", "shortString", "hlPerp", "uInt", "uDecimal", "string"] {
        let hint = Hint::parse(name);
        assert!(!matches!(hint, Hint::Other(_)), "{name} is known");
        assert_eq!(hint.to_string(), name);
    }
    let unknown = Hint::parse("uPercent");
    assert_eq!(unknown, Hint::Other("uPercent".to_owned()));
    assert_eq!(unknown.check("share", "anything, 0.5% or -3"), Ok(()), "checked as free text");
    assert_eq!(
        unknown.check("share", "a|b").map_err(|bad| bad.to_string()),
        Err("share cannot hold {, } or |".to_owned()),
        "and free text still keeps the rules every value keeps"
    );
}

#[test]
fn each_hint_refuses_what_the_venue_refuses_in_words() {
    let date_time = "takes YYYYMMDD-HHMM in UTC";
    let date = "takes YYYYMMDD in UTC";
    let uint = "takes a whole number with no sign or leading zeros";
    let udecimal = "takes a decimal with no sign, no exponent and no leading or trailing zeros";
    let long_short = "x".repeat(11);
    let too_long = "x".repeat(101);
    let cases: Vec<(Hint, &str, &str, Option<&str>)> = vec![
        (Hint::DateTime, "time", "20261001-1200", None),
        (Hint::DateTime, "time", "20280229-1200", None),
        (Hint::DateTime, "time", "20000229-0000", None),
        (Hint::DateTime, "time", "20261231-2359", None),
        (Hint::DateTime, "time", "20261001", Some(date_time)),
        (Hint::DateTime, "time", "2026-10-01 12:00", Some(date_time)),
        (Hint::DateTime, "time", "20261001-2400", Some(date_time)),
        (Hint::DateTime, "time", "20261001-1260", Some(date_time)),
        (Hint::DateTime, "time", "20260231-1200", Some(date_time)),
        (Hint::DateTime, "time", "20250229-1200", Some(date_time)),
        (Hint::DateTime, "time", "21000229-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261301-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261000-1200", Some(date_time)),
        (Hint::DateTime, "time", " 20261001-1200", Some(date_time)),
        (Hint::DateTime, "time", "2026100a-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261001+1200", Some(date_time)),
        (Hint::DateTime, "time", "20261001-120", Some(date_time)),
        (Hint::Date, "day", "20261001", None),
        (Hint::Date, "day", "20240229", None),
        (Hint::Date, "day", "20261001-1200", Some(date)),
        (Hint::Date, "day", "20261301", Some(date)),
        (Hint::Date, "day", "2026101", Some(date)),
        (Hint::Date, "day", "20230229", Some(date)),
        (Hint::ShortString, "shortNameA", "BRA", None),
        (Hint::ShortString, "shortNameA", "0123456789", None),
        (Hint::ShortString, "shortNameA", long_short.as_str(), Some("takes at most 10 characters")),
        (Hint::HlPerp, "perp", "BTC", None),
        (Hint::HlPerp, "perp", "", Some("needs a value")),
        (Hint::UInt, "seconds", "0", None),
        (Hint::UInt, "seconds", "60", None),
        (Hint::UInt, "seconds", "18446744073709551615", None),
        (Hint::UInt, "seconds", "18446744073709551616", Some("takes a whole number that fits in 64 bits")),
        (Hint::UInt, "seconds", "060", Some(uint)),
        (Hint::UInt, "seconds", "00", Some(uint)),
        (Hint::UInt, "seconds", "+60", Some(uint)),
        (Hint::UInt, "seconds", "-1", Some(uint)),
        (Hint::UInt, "seconds", "6e1", Some(uint)),
        (Hint::UInt, "seconds", "6.0", Some(uint)),
        (Hint::UInt, "seconds", " 60", Some(uint)),
        (Hint::UDecimal, "threshold", "0", None),
        (Hint::UDecimal, "threshold", "83114", None),
        (Hint::UDecimal, "threshold", "4460.9", None),
        (Hint::UDecimal, "threshold", "0.5", None),
        (Hint::UDecimal, "threshold", "0.05", None),
        (Hint::UDecimal, "threshold", "4460.90", Some(udecimal)),
        (Hint::UDecimal, "threshold", ".5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "05", Some(udecimal)),
        (Hint::UDecimal, "threshold", "00.5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1.", Some(udecimal)),
        (Hint::UDecimal, "threshold", "0.0", Some(udecimal)),
        (Hint::UDecimal, "threshold", "-1", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1e5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1,000", Some(udecimal)),
        (Hint::String, "priceDescription", "BTC-USDC perp mark", None),
        (Hint::String, "season", "2026/27", None),
        (Hint::String, "priceDescription", "a {b}", Some("cannot hold {, } or |")),
        (Hint::String, "priceDescription", "", Some("needs a value")),
        (Hint::String, "priceDescription", too_long.as_str(), Some("takes at most 100 characters")),
    ];
    for (hint, keyword, value, expected) in cases {
        let got = hint.check(keyword, value).map_err(|bad| bad.to_string());
        let expected = expected.map(|words| format!("{keyword} {words}"));
        assert_eq!(got.err(), expected, "{hint} {value:?}");
    }
}

#[test]
fn a_date_time_is_after_now_and_within_366_days_of_it() {
    let at = |value: &str| Hint::DateTime.check_at("time", value, NOW_MS).map_err(|bad| bad.to_string());
    assert_eq!(at("20260930-1201"), Ok(()));
    assert_eq!(at("20261231-1200"), Ok(()));
    assert_eq!(at("20271001-1200"), Ok(()), "366 days to the minute");
    assert_eq!(at("20271001-1201"), Err("time is more than a year away".to_owned()));
    assert_eq!(at("20260930-1200"), Err("time has already passed".to_owned()), "now itself is not after now");
    assert_eq!(at("20260901-1200"), Err("time has already passed".to_owned()));
    assert_eq!(at("20261001"), Err("time takes YYYYMMDD-HHMM in UTC".to_owned()), "the shape is checked first");
    assert_eq!(Hint::DateTime.check("time", "20200101-0000"), Ok(()), "without a clock only the shape is checked");
    assert_eq!(Hint::Date.check_at("day", "20200101", NOW_MS), Ok(()), "a date has no rule on when");
}

#[test]
fn a_template_instance_is_checked_whole() {
    let declared = || {
        [
            ("perp", Hint::parse("hlPerp")),
            ("priceDescription", Hint::parse("string")),
            ("seconds", Hint::parse("uInt")),
            ("threshold", Hint::parse("uDecimal")),
            ("time", Hint::parse("dateTime")),
        ]
    };
    let good = keywords(&[
        ("perp", "BTC"),
        ("priceDescription", "BTC-USDC perp mark"),
        ("seconds", "60"),
        ("threshold", "83114"),
        ("time", "20261231-1200"),
    ]);
    assert_eq!(good.check(declared(), NOW_MS), []);
    let bad = keywords(&[
        ("perp", "BTC"),
        ("seconds", "060"),
        ("threshold", "83114"),
        ("time", "20271001-1201"),
        ("colour", "red"),
    ]);
    let problems: Vec<String> = bad.check(declared(), NOW_MS).iter().map(ToString::to_string).collect();
    assert_eq!(
        problems,
        [
            "priceDescription needs a value",
            "seconds takes a whole number with no sign or leading zeros",
            "time is more than a year away",
            "colour is not a keyword of this template",
        ]
    );
    assert_eq!(Keywords::new().check([], NOW_MS), [], "a template with no keywords takes none");
}

#[test]
fn a_fee_scale_and_a_fraction_are_in_range_and_sent_in_normal_form() {
    for (typed, sent) in [
        ("0", "0"),
        ("1", "1"),
        ("1.0", "1"),
        ("01", "1"),
        (".50", "0.5"),
        ("2.25", "2.25"),
        ("10", "10"),
        ("10.000", "10"),
    ] {
        let scale: FeeScale = typed.parse().unwrap();
        assert_eq!(scale.to_wire(), sent, "{typed}");
        assert_eq!(scale.to_string(), sent);
    }
    for refused in ["10.01", "11", "-1", "1e1", "", "abc", "+1", "1,5"] {
        assert_eq!(refused.parse::<FeeScale>(), Err(DeployError::FeeScale(refused.to_owned())), "{refused}");
    }
    assert_eq!(
        "10.01".parse::<FeeScale>().unwrap_err().to_string(),
        "a fee scale is a decimal from 0 to 10, which `10.01` is not"
    );
    assert_eq!(FeeScale::ONE, "1.0".parse::<FeeScale>().unwrap());

    for (typed, sent, whole) in [
        ("0", "0", true),
        ("1", "1", true),
        ("1.0", "1", true),
        ("0.000", "0", true),
        ("0.50", "0.5", false),
        (".25", "0.25", false),
        ("0.999999", "0.999999", false),
    ] {
        let fraction: Fraction = typed.parse().unwrap();
        assert_eq!(fraction.to_wire(), sent, "{typed}");
        assert_eq!(fraction.is_whole(), whole, "{typed}");
    }
    for refused in ["1.01", "2", "-0.5", "0.5e0", "", "one"] {
        assert_eq!(refused.parse::<Fraction>(), Err(DeployError::Fraction(refused.to_owned())), "{refused}");
    }
    assert_eq!(Fraction::ONE, "1.000".parse::<Fraction>().unwrap());
}

#[test]
fn a_venue_is_two_to_four_lower_case_letters_and_never_spot() {
    for name in ["ab", "out", "zzz", "abcd"] {
        assert_eq!(venue(name).as_str(), name);
    }
    for refused in ["", "a", "abcde", "AB", "Ab", "a1", "a b", " ab", "\u{e9}t\u{e9}"] {
        assert_eq!(refused.parse::<Venue>(), Err(DeployError::Venue(refused.to_owned())), "{refused:?}");
    }
    assert_eq!(
        "spot".parse::<Venue>().unwrap_err().to_string(),
        "`spot` is the spot venue's name and cannot be claimed"
    );
}

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

/// The venue's own templates (testnet, 2026-10-08): the Python SDK's two-keyword
/// `sportsContestResult` hashes, but the venue's template asks for eight more. Mainnet's
/// `binaryPrice` takes the recorded preset whole.
#[test]
fn the_venue_s_own_template_asks_for_every_keyword_it_declares() {
    let declared = |templates: &[crate::markets::outcomes::OutcomeTemplate], id: &str| -> Vec<(String, Hint)> {
        let template = templates.iter().find(|one| one.id == id).unwrap();
        template.keywords.iter().map(|keyword| (keyword.name.clone(), keyword.hint())).collect()
    };
    let testnet = serde_json::from_str::<Templates>(TESTNET_TEMPLATES).unwrap().0;
    let result = declared(&testnet, "sportsContestResult");
    let typed = keywords(&[("participantB", "Spain"), ("participantA", "Brazil")]);
    let missing: Vec<String> = typed
        .check(result.iter().map(|(name, hint)| (name.as_str(), hint.clone())), NOW_MS)
        .into_iter()
        .filter(|bad| bad.problem == Problem::Missing)
        .map(|bad| bad.keyword)
        .collect();
    assert_eq!(
        missing,
        [
            "competition",
            "contestType",
            "officialSource",
            "resolutionDeadline",
            "scheduledStart",
            "season",
            "sport",
            "stage"
        ]
    );
    let mainnet = serde_json::from_str::<Templates>(MAINNET_TEMPLATES).unwrap().0;
    let binary = declared(&mainnet, "binaryPrice");
    let preset = keywords(&[
        ("threshold", "83114"),
        ("time", "20261231-1200"),
        ("seconds", "60"),
        ("priceDescription", "BTC-USDC perp mark"),
        ("perp", "BTC"),
    ]);
    assert_eq!(preset.check(binary.iter().map(|(name, hint)| (name.as_str(), hint.clone())), NOW_MS), []);
}

#[test]
fn a_question_needs_a_named_outcome_and_a_grant_list_a_grant() {
    assert_eq!(
        OutcomeDeploy::register_question(venue("zzz"), instance("sportsContestResult", &[]), FeeScale::ZERO, vec![]),
        Err(DeployError::NoNamedOutcomes)
    );
    assert_eq!(OutcomeDeploy::set_sub_deployers(venue("zzz"), vec![]), Err(DeployError::NoGrants));
}

#[test]
fn a_grant_names_its_user_in_lower_case() {
    let grants = OutcomeDeploy::set_sub_deployers(
        venue("ab"),
        Permission::ALL
            .into_iter()
            .map(|permission| Grant {
                permission,
                user: address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414"),
                allowed: permission != Permission::SettleQuestion,
            })
            .collect(),
    )
    .unwrap();
    assert_eq!(
        grants.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"setSubDeployers":[{"variant":"registerStandaloneOutcomeFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"registerQuestionFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"registerAndAssociateNamedOutcomeFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"settleOutcome","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"settleQuestion","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":false}]}}"#
    );
}

#[test]
fn each_operation_names_the_grant_a_sub_deployer_needs_for_it() {
    let cases = [
        (python_standalone(), Some(Permission::RegisterStandalone), false),
        (python_question(), Some(Permission::RegisterQuestion), false),
        (python_associate(), Some(Permission::RegisterAndAssociate), false),
        (python_settle_outcome(), Some(Permission::SettleOutcome), true),
        (python_settle_question(), Some(Permission::SettleQuestion), true),
        (python_sub_deployers(), None, false),
    ];
    for (action, permission, settles) in cases {
        let operation = &action.operation;
        assert_eq!(operation.permission(), permission, "{}", operation.wire_name());
        assert_eq!(operation.is_settlement(), settles, "{}", operation.wire_name());
        assert_eq!(
            action.action().get("operation"),
            Some(&Value::Map(Map::new().put(operation.wire_name(), operation.body())))
        );
    }
    assert_eq!(python_settle_question().operation.wire_name(), "settleQuestion2");
    assert_eq!(Permission::SettleQuestion.wire(), "settleQuestion");
}
