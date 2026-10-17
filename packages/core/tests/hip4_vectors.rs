//! Every HIP-4 vector on file, rebuilt through the typed builders (never from the JSON itself) and signed on
//! the network each was made for. The files are in `vectors/`, each naming what made it.
//!
//! - `outcome_deploy.json`: deployer vectors (Hyperliquid's Python SDK, an independent builder, recorded
//!   presets), every one built by the deploy constructors from the inputs as typed. Each keyword is checked
//!   against the hint the venue's own templates give a keyword of that name, and each settlement copies an
//!   `outcomeMeta` listing what it settles.
//! - `outcomes.json`: a first HIP-4 session through an outcome ticket, signed by `@nktkas/hyperliquid` 0.24.3,
//!   on mainnet.
//! - `l1.json`: the HIP-4 market orders among the recorded L1 cases, mainnet.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use df_hyperliquid_core::account::{LocalKey, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::deploy::outcomes::{
    ActivateOutcomeDeployer, FeeScale, Fraction, Grant, Hint, Instance, Keywords, OutcomeDeploy, Permission, Venue,
};
use df_hyperliquid_core::markets::outcomes::px::{Px, Qty};
use df_hyperliquid_core::markets::outcomes::{Coin, Fields, Outcome, OutcomeMeta, Question, SideSpec};
use df_hyperliquid_core::trade::builder::fee::BuilderFee;
use df_hyperliquid_core::trade::{Builder, Cancel, PlaceOrder, Side, Tif};
use df_hyperliquid_core::utils::sign::user_signed::{NonceKey, UserSigned};
use df_hyperliquid_core::utils::sign::{Field, L1Action, Map, Value, connection_id};
use df_hyperliquid_core::utils::{Address, Json, OutcomeSide, hex};

const DEPLOY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcome_deploy.json"));
const OUTCOMES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcomes.json"));
const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));
const TESTNET_META: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_meta_testnet.json"));

/// Hardhat's published accounts #0 and #1, `outcomes.json`'s owner and agent.
const OWNER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// 2026-08-01 00:00 UTC: before every time the deployer cases type, and
/// within a year of each.
const NOW: u64 = 1_785_542_400_000;

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn number(json: &Json, key: &str) -> u64 {
    json.get(key).and_then(Json::as_u64).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn list<'a>(json: &'a Json, key: &str) -> &'a [Json] {
    json.get(key).and_then(Json::as_array).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

fn dec(text: &str) -> df_hyperliquid_core::utils::Dec {
    df_hyperliquid_core::utils::Dec::parse(text).unwrap()
}

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

fn check_signed(name: &str, case: &Json, action: &dyn L1Action, key: &LocalKey, network: Network) {
    let nonce = number(case, "nonce");
    assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}: the action");
    let digest = action.digest(nonce, None, network);
    let signature = key.sign_digest(&digest).unwrap();
    let expected = case.get("signature").unwrap();
    assert_eq!(signature.r_hex(), text(expected, "r"), "{name}: r");
    assert_eq!(signature.s_hex(), text(expected, "s"), "{name}: s");
    assert_eq!(u64::from(signature.v()), number(expected, "v"), "{name}: v");
    assert_eq!(recover(&digest, &signature), Ok(key.address()), "{name}");
    // The recorded body has no `vaultAddress`; this crate posts it as null, which the venue reads as none.
    // Everything else is byte for byte.
    let posted = text(case, "body");
    let with_vault = format!("{},\"vaultAddress\":null}}", &posted[..posted.len() - 1]);
    assert_eq!(action.body(nonce, &signature, None), with_vault, "{name}: the body");
}

#[test]
fn a_first_outcome_session_is_reproduced_on_mainnet() {
    let file = parse(OUTCOMES);
    let owner = LocalKey::from_hex(OWNER_KEY).unwrap();
    let agent = LocalKey::from_hex(AGENT_KEY).unwrap();
    assert_eq!(owner.address(), address(text(&file, "owner")));
    assert_eq!(agent.address(), address(text(&file, "agent")));
    let cases = list(&file, "cases");
    assert_eq!(cases.len(), 6);
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let builder_address = address("0x6967d7acd1ec5f210e9590666498e0cc5d13a843");

    // The approvals, user-signed by the owner.
    let approve = case("approve_agent_atlas");
    let nonce = number(approve, "nonce");
    let agent_approval = UserSigned::new("approveAgent", "HyperliquidTransaction:ApproveAgent", NonceKey::Nonce, nonce)
        .field("agentAddress", Field::Address(agent.address()))
        .field("agentName", Field::String("atlas"));
    let fee_cap = BuilderFee::from_tenths(1_000).percent();
    let fee_nonce = number(case("approve_builder_fee"), "nonce");
    let fee_approval =
        UserSigned::new("approveBuilderFee", "HyperliquidTransaction:ApproveBuilderFee", NonceKey::Nonce, fee_nonce)
            .field("maxFeeRate", Field::String(&fee_cap))
            .field("builder", Field::Address(builder_address));
    for (name, signed) in [("approve_agent_atlas", &agent_approval), ("approve_builder_fee", &fee_approval)] {
        let case = case(name);
        assert_eq!(signed.wire(Network::Mainnet), case.get("action").unwrap().compact(), "{name}");
        let signature = owner.sign_digest(&signed.digest(Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(
            (signature.r_hex(), signature.s_hex()),
            (text(expected, "r").to_owned(), text(expected, "s").to_owned())
        );
        assert_eq!(signed.body(&signature, Network::Mainnet), text(case, "body"), "{name}: the body");
    }

    // The orders, L1 actions the agent signs, every one through the builder.
    let builder = Builder::new(builder_address, BuilderFee::from_tenths(100));
    let yes = Coin::new(1473, OutcomeSide::Yes);
    let no = yes.opposite();
    let bound = yes.market_within(Side::Buy, px("0.6"), dec("0.02"), Qty::new(1)).unwrap().price();
    let ten_dollars = Qty::spend(dec("10"), Px::from_dec(bound).unwrap()).unwrap();
    assert_eq!(ten_dollars, Qty::new(16));
    let buy =
        PlaceOrder::one(yes.market_within(Side::Buy, px("0.6"), dec("0.02"), ten_dollars).unwrap()).through(builder);
    let rest = PlaceOrder::one(no.order(Side::Buy, px("0.385"), Qty::new(30), Tif::Gtc).unwrap()).through(builder);
    let close =
        PlaceOrder::one(yes.market_within(Side::Sell, px("0.58"), dec("0.02"), ten_dollars).unwrap()).through(builder);
    for (name, order) in [("yes_market_buy", &buy), ("no_limit_buy", &rest), ("yes_close", &close)] {
        assert_eq!(order.over_builder_cap(), None, "{name}: 0.1% is under an outcome's 1%");
        check_signed(name, case(name), order, &agent, Network::Mainnet);
    }
    let cancel = Cancel::one(no.asset().unwrap(), 2);
    check_signed("no_cancel", case("no_cancel"), &cancel, &agent, Network::Mainnet);
}

#[test]
fn outcome_market_orders_are_frontend_market_at_the_edges() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let cases = list(&file, "cases");
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let orders = [
        ("order_hip4_frontend_market_buy", Coin::new(1234, OutcomeSide::Yes).market(Side::Buy, Qty::new(25))),
        ("order_hip4_frontend_market_sell", Coin::new(1234, OutcomeSide::No).market(Side::Sell, Qty::new(10))),
    ];
    for (name, order) in orders {
        let case = case(name);
        assert_eq!(case.get("isTestnet"), Some(&Json::Bool(false)), "{name} was made for mainnet");
        let action = PlaceOrder::one(order.unwrap());
        let nonce = number(case, "nonce");
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}");
        assert_eq!(hex::encode_prefixed(&action.msgpack()), text(case, "msgpack"), "{name}: the msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        let signature = key.sign_digest(&action.digest(nonce, None, Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), text(expected, "r"), "{name}: r");
        assert_eq!(signature.s_hex(), text(expected, "s"), "{name}: s");
        assert_eq!(u64::from(signature.v()), number(expected, "v"), "{name}: v");
    }
}

/// On testnet the outcome ids are testnet's own: the order below names
/// testnet's daily BTC market by the id its recorded `outcomeMeta` gives,
/// and is signed for testnet. An L1 hash names no network; the digest does,
/// so the same action signed for mainnet is a different signature.
#[test]
fn a_testnet_order_names_testnet_s_own_outcome_and_signs_for_testnet() {
    let meta: OutcomeMeta = serde_json::from_str(TESTNET_META).unwrap();
    let daily = meta
        .outcomes
        .iter()
        .find(|row| {
            let fields = Fields::parse(&row.description);
            let field = |key: &str| fields.text(key);
            field("underlying").as_deref() == Some("BTC")
                && field("targetPrice").as_deref() == Some("83136")
                && field("expiry").as_deref() == Some("20261009-0300")
        })
        .expect("testnet's daily BTC market");
    let order = daily.coin(OutcomeSide::Yes).market(Side::Buy, Qty::new(25)).unwrap();
    assert_eq!(order.asset().id(), 100_231_360, "from testnet's data, not a mainnet constant");
    let action = PlaceOrder::one(order);
    let key = LocalKey::from_hex(AGENT_KEY).unwrap();
    let nonce = 1_791_417_600_000;
    let testnet = action.digest(nonce, None, Network::Testnet);
    let signature = key.sign_digest(&testnet).unwrap();
    assert_eq!(recover(&testnet, &signature), Ok(key.address()));
    let mainnet = action.digest(nonce, None, Network::Mainnet);
    assert_ne!(testnet, mainnet);
    assert_ne!(recover(&mainnet, &signature), Ok(key.address()), "a testnet signature is no mainnet one");
}
