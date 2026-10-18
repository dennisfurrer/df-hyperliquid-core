//! `outcome_deploy.json`: the Python SDK's deployer hashes and their multisig wrapping, an independent
//! builder's bytes and mainnet digests, and the recorded presets, each built from its inputs as typed.

use std::collections::BTreeMap;

use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::deploy::outcomes::{
    ActivateOutcomeDeployer, FeeScale, Fraction, Grant, Hint, Instance, Keywords, OutcomeDeploy, Permission, Venue,
};
use df_hyperliquid_core::markets::outcomes::{Outcome, OutcomeMeta, Question, SideSpec};
use df_hyperliquid_core::utils::sign::{L1Action, Map, Value, connection_id};
use df_hyperliquid_core::utils::{Json, hex};

use super::{address, list, number, parse, text};

const DEPLOY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcome_deploy.json"));

/// 2026-08-01 00:00 UTC: before every time the deployer cases type, and
/// within a year of each.
const NOW: u64 = 1_785_542_400_000;

/// The hint the venue's templates give a keyword of this name (mainnet
/// `binaryPrice`, testnet `binaryPrice4` and `sportsContestResult`, as
/// recorded in `fixtures/`); free text otherwise.
fn hint(keyword: &str) -> &'static str {
    match keyword {
        "time" | "resolutionDeadline" | "scheduledStart" => "dateTime",
        "perp" => "hlPerp",
        "seconds" => "uInt",
        "threshold" => "uDecimal",
        _ => "string",
    }
}

/// A template instance from a case's `template` and `keywords`, as typed, every value one its
/// keyword's hint takes at [`NOW`].
fn instance(name: &str, json: &Json) -> Instance {
    let pairs = list(json, "keywords").iter().map(|pair| {
        let pair = pair.as_array().unwrap();
        (pair[0].as_str().unwrap().to_owned(), pair[1].as_str().unwrap().to_owned())
    });
    let keywords = Keywords::from_pairs(pairs).unwrap_or_else(|bad| panic!("{name}: {bad}"));
    let declared: Vec<(&str, Hint)> =
        keywords.iter().map(|(keyword, _)| (keyword, Hint::parse(hint(keyword)))).collect();
    assert_eq!(keywords.check(declared, NOW), [], "{name}: the keywords");
    Instance::new(text(json, "template"), keywords)
}

fn row(outcome: u32, venue: &str, name: &str, description: &str, sides: [&str; 2]) -> Outcome {
    Outcome {
        outcome,
        name: name.to_owned(),
        description: description.to_owned(),
        side_specs: sides.iter().map(|side| SideSpec { name: (*side).to_owned() }).collect(),
        quote_token: Some("USDC".to_owned()),
        venue: Some(venue.to_owned()),
        deployer_fee_scale: None,
    }
}

fn sides(json: &Json) -> [String; 2] {
    let both = list(json, "sides");
    [both[0].as_str().unwrap().to_owned(), both[1].as_str().unwrap().to_owned()]
}

fn settled_row(venue: &str, settlement: &Json) -> Outcome {
    let [first, second] = sides(settlement);
    row(
        u32::try_from(number(settlement, "outcome")).unwrap(),
        venue,
        text(settlement, "name"),
        text(settlement, "description"),
        [&first, &second],
    )
}

/// A case's action, built from its inputs as typed.
fn built(case: &Json) -> Box<dyn L1Action> {
    let name = text(case, "name");
    let venue_name = text(case, "venue");
    let venue: Venue = venue_name.parse().unwrap();
    let given = case.get("given").unwrap();
    let fee = || text(given, "feeScale").parse::<FeeScale>().unwrap();
    let id = |key: &str| u32::try_from(number(given, key)).unwrap();
    match name {
        "activate" => Box::new(ActivateOutcomeDeployer::Activate(venue)),
        "deactivate" => Box::new(ActivateOutcomeDeployer::Deactivate),
        _ if given.get("grants").is_some() => {
            let grants: Vec<Grant> = list(given, "grants")
                .iter()
                .map(|grant| Grant {
                    permission: Permission::ALL
                        .into_iter()
                        .find(|one| one.wire() == text(grant, "permission"))
                        .unwrap(),
                    user: address(text(grant, "user")),
                    allowed: grant.get("allowed") == Some(&Json::Bool(true)),
                })
                .collect();
            Box::new(OutcomeDeploy::set_sub_deployers(venue, grants).unwrap())
        }
        _ if given.get("settlements").is_some() => {
            let settlements = list(given, "settlements");
            let outcomes: Vec<Outcome> = settlements.iter().map(|one| settled_row(venue_name, one)).collect();
            let meta = OutcomeMeta {
                questions: vec![Question {
                    question: id("question"),
                    name: text(given, "name").to_owned(),
                    description: text(given, "description").to_owned(),
                    fallback_outcome: None,
                    named_outcomes: outcomes.iter().map(|row| row.outcome).collect(),
                    settled_named_outcomes: Vec::new(),
                }],
                outcomes,
                ..OutcomeMeta::default()
            };
            let winner = settlements.iter().find(|one| text(one, "fraction") == "1").unwrap();
            let winner = u32::try_from(number(winner, "outcome")).unwrap();
            Box::new(OutcomeDeploy::settle_question(venue, &meta, id("question"), winner).unwrap())
        }
        _ if given.get("fraction").is_some() => {
            let meta = OutcomeMeta { outcomes: vec![settled_row(venue_name, given)], ..OutcomeMeta::default() };
            let fraction: Fraction = text(given, "fraction").parse().unwrap();
            Box::new(OutcomeDeploy::settle_outcome(venue, &meta, id("outcome"), fraction).unwrap())
        }
        _ if given.get("named").and_then(Json::as_array).is_some() => {
            let question = instance(name, given.get("question").unwrap());
            let named: Vec<Instance> = list(given, "named").iter().map(|one| instance(name, one)).collect();
            Box::new(OutcomeDeploy::register_question(venue, question, fee(), named).unwrap())
        }
        _ if given.get("named").is_some() => {
            let named = instance(name, given.get("named").unwrap());
            Box::new(OutcomeDeploy::register_and_associate(venue, id("question"), named))
        }
        _ => Box::new(OutcomeDeploy::register_standalone(venue, instance(name, given), fee())),
    }
}

#[test]
fn the_python_sdk_s_deployer_hashes_come_out_of_the_builders() {
    let file = parse(DEPLOY);
    let python = file.get("python").unwrap();
    let nonce = number(python, "nonce");
    let cases = list(python, "cases");
    assert_eq!(cases.len(), 8);
    let mut actions = BTreeMap::new();
    for case in cases {
        let name = text(case, "name");
        let action = built(case);
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}: the action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        actions.insert(name.to_owned(), action);
    }

    let multi = python.get("multiSig").unwrap();
    let (user, outer) = (text(multi, "multiSigUser"), text(multi, "outerSigner"));
    for inner in list(multi, "inner") {
        let name = text(inner, "name");
        let packed = Value::Array(vec![
            Value::from(user.to_owned()),
            Value::from(outer.to_owned()),
            Value::Map(actions[name].action()),
        ])
        .to_msgpack();
        assert_eq!(hex::encode_prefixed(&connection_id(&packed, nonce, None)), text(inner, "actionHash"), "{name}");
    }
    let envelope = multi.get("envelope").unwrap();
    let signatures = list(envelope, "signatures").iter().map(|signature| {
        Value::Map(
            Map::new()
                .put("r", text(signature, "r").to_owned())
                .put("s", text(signature, "s").to_owned())
                .put("v", number(signature, "v")),
        )
    });
    let wrapped = Map::new()
        .put("signatureChainId", text(envelope, "signatureChainId").to_owned())
        .put("signatures", Value::Array(signatures.collect()))
        .put(
            "payload",
            Map::new()
                .put("multiSigUser", user.to_owned())
                .put("outerSigner", outer.to_owned())
                .put("action", actions[text(envelope, "name")].action()),
        );
    assert_eq!(
        text(envelope, "signatureChainId"),
        Network::Testnet.signature_chain_id_hex(),
        "the envelope names testnet's chain"
    );
    assert_eq!(hex::encode_prefixed(&connection_id(&wrapped.to_msgpack(), nonce, None)), text(envelope, "actionHash"));
}

#[test]
fn independent_bytes_hashes_and_mainnet_digests_come_out_of_the_builders() {
    let file = parse(DEPLOY);
    let independent = file.get("independent").unwrap();
    let nonce = number(independent, "nonce");
    let cases = list(independent, "cases");
    assert_eq!(cases.len(), 3);
    for case in cases {
        let name = text(case, "name");
        let action = built(case);
        assert_eq!(hex::encode(&action.msgpack()), text(case, "msgpack"), "{name}: the msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        let digest = action.digest(nonce, None, Network::Mainnet);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "mainnetDigest"), "{name}: the digest");
        assert_ne!(action.digest(nonce, None, Network::Testnet), digest, "{name}: testnet signs another digest");
    }
}

#[test]
fn the_recorded_presets_come_out_of_the_builders_byte_for_byte() {
    let file = parse(DEPLOY);
    let cases = list(file.get("studio").unwrap(), "cases");
    assert_eq!(cases.len(), 8);
    for case in cases {
        let name = text(case, "name");
        let action = built(case);
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}");
        if let Some(Json::String(packed)) = case.get("msgpack") {
            assert_eq!(hex::encode(&action.msgpack()), *packed, "{name}: the msgpack");
        }
    }
}
