//! Every HIP-4 vector on file, reproduced through hl-hip4 and hl-trade's
//! typed builders, never through the JSON itself, and signed on the network
//! each was made for. The files are in `vectors/`, each naming what made it.
//!
//! - `outcome_deploy.json`: hip4-tui's deployer vectors (Hyperliquid's
//!   Python SDK, Outcome's `drs-admin`, Studio's presets), every one built by
//!   the deployer flow ([`hl_hip4::flow`]) from the inputs as typed.
//! - `outcomes.json`: atlas's first HIP-4 session through its outcome ticket,
//!   signed by `@nktkas/hyperliquid` 0.24.3, on mainnet.
//! - `l1.json`: the HIP-4 market orders among atlas's L1 cases, mainnet.
//!
//! The deployer flow is checked against a desk made for each case: the
//! templates declare exactly the keywords the case types, each with the hint
//! the venue's own templates give a keyword of that name, and `outcomeMeta`
//! lists what a settle or grant copies from.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use hl_core::{
    Address, BuilderFee, Field, Json, L1Action, LocalKey, Map, Network, OutcomeSide, Value, connection_id, hex,
    recover,
    user_signed::{NonceKey, UserSigned},
};
use hl_hip4::{
    Coin, Deployer, FeeScale, Fraction, Grant, Outcome, OutcomeMeta, Permission, Question, SideSpec, Venue,
    flow::{self, Desk, Filled, Plan, Seat},
    market::Catalog,
    px::{Px, Qty},
    template::{Keyword, OutcomeTemplate, TemplateRole},
};
use hl_trade::{Builder, Cancel, PlaceOrder, Side, Tif};

const DEPLOY: &str = include_str!("../../../vectors/outcome_deploy.json");
const OUTCOMES: &str = include_str!("../../../vectors/outcomes.json");
const L1: &str = include_str!("../../../vectors/l1.json");
const TESTNET_META: &str = include_str!("../fixtures/outcome_meta_testnet.json");

/// Hardhat's published accounts #0 and #1, `outcomes.json`'s owner and agent.
const OWNER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// 2026-08-01 00:00 UTC: before every time the deployer cases type, and
/// within a year of each.
const DESK_NOW: u64 = 1_785_542_400_000;

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

fn dec(text: &str) -> hl_core::Dec {
    hl_core::Dec::parse(text).unwrap()
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

fn filled(json: &Json) -> Filled {
    let pairs = list(json, "keywords").iter().map(|pair| {
        let pair = pair.as_array().unwrap();
        (pair[0].as_str().unwrap().to_owned(), pair[1].as_str().unwrap().to_owned())
    });
    Filled::new(text(json, "template"), pairs.collect::<Vec<_>>())
}

fn template(filled: &Filled, role: TemplateRole) -> OutcomeTemplate {
    OutcomeTemplate {
        id: filled.template.clone(),
        role,
        name: String::new(),
        description: String::new(),
        keywords: filled
            .values
            .keys()
            .map(|name| Keyword { name: name.clone(), kind: hint(name).to_owned() })
            .collect(),
    }
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

/// The question a named outcome is added to: live on the venue, made from
/// the template the added outcome's names as its parent.
const PARENT: &str = "sportsContestResult";

type Step<'s> = Box<dyn FnOnce(&Desk<'_>) -> Result<Plan, Vec<flow::Refusal>> + 's>;

fn run(name: &str, meta: &OutcomeMeta, templates: &[OutcomeTemplate], seat: &Seat, step: Step<'_>) -> Plan {
    step(&Desk { meta, templates, seat, now_ms: DESK_NOW, limits: None })
        .unwrap_or_else(|refused| panic!("{name}: {refused:?}"))
}

/// Builds one case through the deployer flow, against a desk made for it.
fn through_the_flow(case: &Json) -> Plan {
    let name = text(case, "name");
    let venue = text(case, "venue");
    let given = case.get("given").unwrap();
    let deployer = Address::from_bytes([0x0d; 20]);
    let mut meta = OutcomeMeta {
        deployers: vec![Deployer { deployer, venue: Some(venue.to_owned()), sub_deployers: Vec::new() }],
        ..OutcomeMeta::default()
    };
    let mut templates = Vec::new();
    let seat = Seat::Deployer { venue: venue.to_owned() };
    let fee = || text(given, "feeScale").parse::<FeeScale>().unwrap();
    let run = |meta: &OutcomeMeta, templates: &[OutcomeTemplate], seat: &Seat, step: Step<'_>| {
        run(name, meta, templates, seat, step)
    };
    match name {
        "activate" => {
            meta.deployers.clear();
            let venue: Venue = venue.parse().unwrap();
            run(&meta, &templates, &Seat::Free, Box::new(move |desk| flow::activate(desk, venue)))
        }
        "deactivate" => run(&meta, &templates, &seat, Box::new(flow::deactivate)),
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
            // What is to be taken away is held now; what is to be given is not.
            meta.deployers[0].sub_deployers = grants
                .iter()
                .filter(|grant| !grant.allowed)
                .map(|grant| (grant.permission.wire().to_owned(), vec![grant.user]))
                .collect();
            run(&meta, &templates, &seat, Box::new(|desk| flow::set_grants(desk, &grants)))
        }
        _ if given.get("settlements").is_some() => {
            let question = u32::try_from(number(given, "question")).unwrap();
            let settlements = list(given, "settlements");
            meta.outcomes = settlements.iter().map(|one| settled_row(venue, one)).collect();
            meta.questions = vec![Question {
                question,
                name: text(given, "name").to_owned(),
                description: text(given, "description").to_owned(),
                fallback_outcome: None,
                named_outcomes: meta.outcomes.iter().map(|row| row.outcome).collect(),
                settled_named_outcomes: Vec::new(),
            }];
            let winner = settlements.iter().find(|one| text(one, "fraction") == "1").unwrap();
            let winner = u32::try_from(number(winner, "outcome")).unwrap();
            run(&meta, &templates, &seat, Box::new(|desk| flow::settle_question(desk, question, winner)))
        }
        _ if given.get("fraction").is_some() => {
            meta.outcomes = vec![settled_row(venue, given)];
            let outcome = meta.outcomes[0].outcome;
            let fraction: Fraction = text(given, "fraction").parse().unwrap();
            run(&meta, &templates, &seat, Box::new(|desk| flow::settle_outcome(desk, outcome, fraction)))
        }
        _ if given.get("named").and_then(Json::as_array).is_some() => {
            let question = filled(given.get("question").unwrap());
            let named: Vec<Filled> = list(given, "named").iter().map(filled).collect();
            templates.push(template(&question, TemplateRole::Question));
            for one in &named {
                templates.push(template(one, TemplateRole::QuestionOutcome { parent: question.template.clone() }));
            }
            run(&meta, &templates, &seat, Box::new(|desk| flow::register_question(desk, &question, fee(), &named)))
        }
        _ if given.get("named").is_some() => {
            let question = u32::try_from(number(given, "question")).unwrap();
            let named = filled(given.get("named").unwrap());
            templates.push(template(&named, TemplateRole::QuestionOutcome { parent: PARENT.to_owned() }));
            meta.outcomes = vec![row(9_999, venue, "template:sportsContestDraw", "", ["Yes", "No"])];
            meta.questions = vec![Question {
                question,
                name: format!("template:{PARENT}"),
                description: String::new(),
                fallback_outcome: None,
                named_outcomes: vec![9_999],
                settled_named_outcomes: Vec::new(),
            }];
            run(&meta, &templates, &seat, Box::new(|desk| flow::add_to_question(desk, question, &named)))
        }
        _ => {
            let outcome = filled(given);
            let role = TemplateRole::Standalone { side_names: vec!["Yes".to_owned(), "No".to_owned()] };
            templates.push(template(&outcome, role));
            run(&meta, &templates, &seat, Box::new(|desk| flow::register_outcome(desk, &outcome, fee())))
        }
    }
}

#[test]
fn the_python_sdk_s_deployer_hashes_come_out_of_the_flow() {
    let file = parse(DEPLOY);
    let python = file.get("python").unwrap();
    let nonce = number(python, "nonce");
    let cases = list(python, "cases");
    assert_eq!(cases.len(), 8);
    let mut built = BTreeMap::new();
    for case in cases {
        let name = text(case, "name");
        let plan = through_the_flow(case);
        assert_eq!(plan.action.wire(), case.get("action").unwrap().compact(), "{name}: the action");
        assert_eq!(hex::encode_prefixed(&plan.action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        built.insert(name.to_owned(), plan.action);
    }

    let multi = python.get("multiSig").unwrap();
    let (user, outer) = (text(multi, "multiSigUser"), text(multi, "outerSigner"));
    for inner in list(multi, "inner") {
        let name = text(inner, "name");
        let packed = Value::Array(vec![
            Value::from(user.to_owned()),
            Value::from(outer.to_owned()),
            Value::Map(built[name].action()),
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
                .put("action", built[text(envelope, "name")].action()),
        );
    assert_eq!(
        text(envelope, "signatureChainId"),
        Network::Testnet.signature_chain_id_hex(),
        "the envelope names testnet's chain"
    );
    assert_eq!(hex::encode_prefixed(&connection_id(&wrapped.to_msgpack(), nonce, None)), text(envelope, "actionHash"));
}

#[test]
fn drs_admin_s_bytes_hashes_and_mainnet_digests_come_out_of_the_flow() {
    let file = parse(DEPLOY);
    let drs = file.get("drsAdmin").unwrap();
    let nonce = number(drs, "nonce");
    let cases = list(drs, "cases");
    assert_eq!(cases.len(), 3);
    for case in cases {
        let name = text(case, "name");
        let action = through_the_flow(case).action;
        assert_eq!(hex::encode(&action.msgpack()), text(case, "msgpack"), "{name}: the msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        let digest = action.digest(nonce, None, Network::Mainnet);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "mainnetDigest"), "{name}: the digest");
        assert_ne!(action.digest(nonce, None, Network::Testnet), digest, "{name}: testnet signs another digest");
    }
}

#[test]
fn studio_s_presets_come_out_of_the_flow_byte_for_byte() {
    let file = parse(DEPLOY);
    let cases = list(file.get("studio").unwrap(), "cases");
    assert_eq!(cases.len(), 8);
    for case in cases {
        let name = text(case, "name");
        let action = through_the_flow(case).action;
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}");
        if let Some(Json::String(packed)) = case.get("msgpack") {
            assert_eq!(hex::encode(&action.msgpack()), *packed, "{name}: the msgpack");
        }
    }
}

/// One signed case: the action as recorded, its signature on `network`, and
/// the body it posts.
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
    // atlas posts no `vaultAddress`; hl-core posts it as null, which the
    // venue reads as none. Everything else is byte for byte.
    let posted = text(case, "body");
    let with_vault = format!("{},\"vaultAddress\":null}}", &posted[..posted.len() - 1]);
    assert_eq!(action.body(nonce, &signature, None), with_vault, "{name}: the body");
}

#[test]
fn atlas_s_first_outcome_session_is_reproduced_on_mainnet() {
    let file = parse(OUTCOMES);
    let owner = LocalKey::from_hex(OWNER_KEY).unwrap();
    let agent = LocalKey::from_hex(AGENT_KEY).unwrap();
    assert_eq!(owner.address(), address(text(&file, "owner")));
    assert_eq!(agent.address(), address(text(&file, "agent")));
    let cases = list(&file, "cases");
    assert_eq!(cases.len(), 6);
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let builder_address = address("0x6967d7acd1ec5f210e9590666498e0cc5d13a843");

    // The approvals, user-signed by the owner, as hl-account builds them.
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
fn atlas_s_outcome_market_orders_are_frontend_market_at_the_edges() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let cases = list(&file, "cases");
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let orders = [
        ("order_hip4_frontend_market_buy", Coin::new(1234, OutcomeSide::Yes).market(Side::Buy, dec("25"))),
        ("order_hip4_frontend_market_sell", Coin::new(1234, OutcomeSide::No).market(Side::Sell, dec("10"))),
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
    let catalog = Catalog::build(&meta, 1_791_417_600_000);
    let daily = catalog
        .markets
        .iter()
        .find(|market| market.title == "BTC above 83136 at 2026-10-09 03:00Z")
        .expect("testnet's daily BTC market");
    let order = daily.sides.yes.market(Side::Buy, dec("25")).unwrap();
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
