//! The venue `out` below is shaped as live ones are (a description ending in a space, a question with a leg
//! already settled); the templates are the venue's as recorded where noted.

use crate::utils::sign::L1Action;
use crate::utils::{Address, Dec};

use super::*;
use crate::deploy::outcomes::actions::{FeeScale, Fraction, Grant, Permission, Problem};
use crate::markets::outcomes::meta::{Deployer, Outcome, OutcomeMeta, Question, SideSpec};
use crate::markets::outcomes::template::{Keyword, OutcomeTemplate, TemplateRole, Templates};

const ME: &str = "0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e";
const SUB: &str = "0x6947a610ef50f8b5d4b59abcd94dd75aaaa645b9";
const TESTNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));
const MAINNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));

/// 2026-09-30 00:00 UTC.
const NOW: u64 = 1_790_726_400_000;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn keyword(name: &str, kind: &str) -> Keyword {
    Keyword { name: name.to_owned(), kind: kind.to_owned() }
}

fn templates() -> Vec<OutcomeTemplate> {
    let template = |id: &str, role: TemplateRole, name: &str, keywords: Vec<Keyword>| OutcomeTemplate {
        id: id.to_owned(),
        role,
        name: name.to_owned(),
        description: String::new(),
        keywords,
    };
    let named = || TemplateRole::QuestionOutcome { parent: "sportsContestResult".to_owned() };
    vec![
        template(
            "binaryPrice",
            TemplateRole::Standalone { side_names: vec!["Yes".to_owned(), "No".to_owned()] },
            "{perp} above {threshold} at {time}?",
            vec![
                keyword("perp", "hlPerp"),
                keyword("priceDescription", "string"),
                keyword("seconds", "uInt"),
                keyword("threshold", "uDecimal"),
                keyword("time", "dateTime"),
            ],
        ),
        template(
            "sportsContestResult",
            TemplateRole::Question,
            "{participantA} v {participantB}",
            vec![keyword("participantA", "string"), keyword("participantB", "string")],
        ),
        template("sportsContestParticipant2", named(), "{participant}", vec![keyword("participant", "string")]),
        template("sportsContestDraw2", named(), "Draw", vec![]),
    ]
}

fn row(outcome: u32, name: &str, description: &str, sides: [&str; 2]) -> Outcome {
    Outcome {
        outcome,
        name: name.to_owned(),
        description: description.to_owned(),
        side_specs: sides.iter().map(|side| SideSpec { name: (*side).to_owned() }).collect(),
        quote_token: Some("USDC".to_owned()),
        venue: Some("out".to_owned()),
        deployer_fee_scale: Some(Dec::ONE),
    }
}

/// The venue `out`, deployed by [`ME`] with [`SUB`] allowed to settle
/// outcomes: a standalone outcome whose description ends in a space, and a
/// question of three named outcomes and a fallback, the first settled.
fn meta() -> OutcomeMeta {
    OutcomeMeta {
        outcomes: vec![
            row(
                1209,
                "template:priceTouch",
                "perp:HYPE|priceDescription:HYPE-USDC perp mark |seconds:1|target:100|time:20261001-0000",
                ["template:Yes", "template:No"],
            ),
            row(1472, "template fallback", "other", ["Yes", "No"]),
            row(1473, "template:sportsContestParticipant2", "participant:Denmark", ["Yes", "No"]),
            row(1474, "template:sportsContestParticipant2", "participant:Portugal", ["Yes", "No"]),
            row(1475, "template:sportsContestDraw2", "", ["Yes", "No"]),
        ],
        questions: vec![Question {
            question: 350,
            name: "template:sportsContestResult".to_owned(),
            description: "participantA:Denmark|participantB:Portugal".to_owned(),
            fallback_outcome: Some(1472),
            named_outcomes: vec![1473, 1474, 1475],
            settled_named_outcomes: vec![1473],
        }],
        deployers: vec![Deployer {
            deployer: address(ME),
            venue: Some("out".to_owned()),
            sub_deployers: vec![("settleOutcome".to_owned(), vec![address(SUB)])],
        }],
        fee_scale: None,
    }
}

fn deployer() -> Seat {
    Seat::Deployer { venue: "out".to_owned() }
}

fn granted(may: &[&str]) -> Seat {
    Seat::Granted {
        venue: "out".to_owned(),
        deployer: address(ME),
        may: may.iter().map(|operation| (*operation).to_owned()).collect(),
    }
}

fn filled(template: &str, pairs: &[(&str, &str)]) -> Filled {
    Filled::new(template, pairs.iter().copied())
}

fn on<T>(seat: &Seat, step: impl FnOnce(&Desk<'_>) -> T) -> T {
    let (meta, templates) = (meta(), templates());
    step(&Desk { meta: &meta, templates: &templates, seat, now_ms: NOW, limits: None })
}

fn said(refused: &[Refusal]) -> String {
    refused.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
}

#[test]
fn the_seat_is_the_account_s_own_venue_then_a_grant_then_none() {
    let meta = meta();
    assert_eq!(seat_of(&meta, address(ME)), deployer());
    assert_eq!(seat_of(&meta, address(SUB)), granted(&["settleOutcome"]));
    assert_eq!(seat_of(&meta, address("0x1111111111111111111111111111111111111111")), Seat::Free);
}

#[test]
fn a_grant_of_settle_question_covers_settle_question2() {
    let seat = granted(&["settleQuestion"]);
    assert!(seat.may("settleQuestion2"));
    assert!(!seat.may("settleOutcome"));
    assert!(deployer().may("anything") && !Seat::Free.may("settleOutcome"));
}

#[test]
fn the_cap_counts_every_row_on_the_venue_fallbacks_included() {
    assert_eq!(live_on(&meta(), "out"), 5);
    assert_eq!(live_on(&meta(), "skew"), 0);
}

#[test]
fn on_chain_a_row_is_its_template_and_its_keywords_sorted() {
    let templates = templates();
    let values = filled(
        "binaryPrice",
        &[
            ("time", "20261001-0000"),
            ("perp", "BTC"),
            ("threshold", "100000"),
            ("seconds", "1"),
            ("priceDescription", "BTC-USDC mark"),
        ],
    );
    let chain = on_chain(&templates[0], &values.values);
    // As outcome 1210 reads on mainnet.
    assert_eq!(chain.name, "template:binaryPrice");
    assert_eq!(
        chain.description,
        "perp:BTC|priceDescription:BTC-USDC mark|seconds:1|threshold:100000|time:20261001-0000"
    );
    assert_eq!(chain.sides, Some(["template:Yes".to_owned(), "template:No".to_owned()]));
    assert_eq!(on_chain(&templates[2], &filled("x", &[]).values).sides, None);
}

#[test]
fn an_outcome_is_built_with_its_keywords_sorted() {
    let typed = filled(
        "binaryPrice",
        &[
            ("time", "20261001-0000"),
            ("threshold", "83114"),
            ("seconds", "60"),
            ("priceDescription", "BTC-USDC perp mark"),
            ("perp", "BTC"),
        ],
    );
    let plan = on(&deployer(), |desk| register_outcome(desk, &typed, "1.0".parse().unwrap())).unwrap();
    assert_eq!(
        plan.action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"registerStandaloneOutcomeFromTemplate":{"id":"binaryPrice","keywordToValue":[["perp","BTC"],["priceDescription","BTC-USDC perp mark"],["seconds","60"],["threshold","83114"],["time","20261001-0000"]],"deployerFeeScale":"1"}}}"#
    );
    assert!(!plan.action.own_key_only());
    assert_eq!(plan.action.request(), "outcomeDeploy");
    assert_eq!(plan.what, "register a binaryPrice outcome on out");
    assert_eq!(
        plan.back,
        Back::Listed {
            venue: "out".to_owned(),
            name: "template:binaryPrice".to_owned(),
            description: "perp:BTC|priceDescription:BTC-USDC perp mark|seconds:60|threshold:83114|time:20261001-0000"
                .to_owned(),
            above: 1475,
        }
    );
}

#[test]
fn an_outcome_not_filled_in_says_every_reason() {
    let typed =
        filled("binaryPrice", &[("perp", "BTC"), ("seconds", "060"), ("time", "20250101-0000"), ("colour", "red")]);
    let refused = on(&deployer(), |desk| register_outcome(desk, &typed, FeeScale::ONE)).unwrap_err();
    let problems: Vec<(Option<usize>, &str, Problem)> = refused
        .iter()
        .filter_map(|why| match why {
            Refusal::Value { named, bad } => Some((*named, bad.keyword.as_str(), bad.problem)),
            _ => None,
        })
        .collect();
    assert_eq!(
        problems,
        [
            (None, "priceDescription", Problem::Missing),
            (None, "seconds", Problem::UInt),
            (None, "threshold", Problem::Missing),
            (None, "time", Problem::Passed),
            (None, "colour", Problem::Unknown),
        ]
    );
    assert_eq!(refused.len(), 5);
}

#[test]
fn a_template_must_be_one_the_venue_offers_for_the_step() {
    let refused = on(&deployer(), |desk| register_outcome(desk, &filled("sportsContestResult", &[]), FeeScale::ONE));
    assert_eq!(
        refused,
        Err(vec![Refusal::NoTemplate { id: "sportsContestResult".to_owned(), role: Role::Standalone }])
    );
    assert_eq!(
        Refusal::NoTemplate { id: "x".to_owned(), role: Role::Named { parent: "y".to_owned() } }.to_string(),
        "no template x makes a named outcome of y"
    );
}

#[test]
fn a_question_keeps_its_named_outcomes_in_the_order_given() {
    let question = filled("sportsContestResult", &[("participantB", "Portugal"), ("participantA", "Denmark")]);
    let named = [
        filled("sportsContestParticipant2", &[("participant", "Denmark")]),
        filled("sportsContestDraw2", &[]),
        filled("sportsContestParticipant2", &[("participant", "Portugal")]),
    ];
    let plan = on(&deployer(), |desk| register_question(desk, &question, FeeScale::ONE, &named)).unwrap();
    assert_eq!(
        plan.action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"registerQuestionFromTemplate":{"questionTemplateInstance":{"id":"sportsContestResult","keywordToValue":[["participantA","Denmark"],["participantB","Portugal"]],"deployerFeeScale":"1"},"namedOutcomeTemplateInstances":[{"id":"sportsContestParticipant2","keywordToValue":[["participant","Denmark"]]},{"id":"sportsContestDraw2","keywordToValue":[]},{"id":"sportsContestParticipant2","keywordToValue":[["participant","Portugal"]]}]}}}"#
    );
    assert_eq!(plan.what, "register a sportsContestResult question with 3 named outcomes on out");
    assert!(matches!(&plan.back, Back::Listed { name, description, .. }
        if name == "template:sportsContestParticipant2" && description == "participant:Denmark"));
}

#[test]
fn a_question_says_which_named_outcome_is_wrong() {
    let question = filled("sportsContestResult", &[("participantA", "Denmark"), ("participantB", "Portugal")]);
    let named =
        [filled("sportsContestParticipant2", &[("participant", "Denmark")]), filled("sportsContestParticipant2", &[])];
    let refused = on(&deployer(), |desk| register_question(desk, &question, FeeScale::ONE, &named)).unwrap_err();
    assert_eq!(said(&refused), "named outcome 2: participant needs a value");
    let none = on(&deployer(), |desk| register_question(desk, &question, FeeScale::ONE, &[])).unwrap_err();
    assert_eq!(none, vec![Refusal::Deploy(crate::deploy::outcomes::actions::DeployError::NoNamedOutcomes)]);
}

#[test]
fn adding_to_a_question_uses_the_templates_its_own_template_takes() {
    let named = filled("sportsContestParticipant2", &[("participant", "Spain")]);
    let plan = on(&deployer(), |desk| add_to_question(desk, 350, &named)).unwrap();
    assert_eq!(
        plan.action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"registerAndAssociateNamedOutcomeFromTemplate":{"question":350,"namedOutcomeTemplateInstance":{"id":"sportsContestParticipant2","keywordToValue":[["participant","Spain"]]}}}}"#
    );
    assert_eq!(on(&deployer(), |desk| add_to_question(desk, 999, &named)).unwrap_err(), vec![Refusal::NoQuestion(999)]);
    let wrong = filled("binaryPrice", &[]);
    assert!(matches!(
        &on(&deployer(), |desk| add_to_question(desk, 350, &wrong)).unwrap_err()[..],
        [Refusal::NoTemplate { role: Role::Named { .. }, .. }]
    ));
}

#[test]
fn what_is_left_to_settle_is_outcomes_outside_questions_then_questions() {
    let meta = meta();
    let ids: Vec<u32> = unsettled(&meta, "out").into_iter().map(Unsettled::id).collect();
    assert_eq!(ids, [1209, 350]);
    assert!(unsettled(&meta, "skew").is_empty());
}

#[test]
fn settling_an_outcome_copies_its_words_exactly_trailing_space_and_all() {
    let plan = on(&deployer(), |desk| settle_outcome(desk, 1209, Fraction::ONE)).unwrap();
    assert_eq!(
        plan.action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleOutcome":{"outcome":1209,"settleFraction":"1","details":"","nameAndDescription":["template:priceTouch","perp:HYPE|priceDescription:HYPE-USDC perp mark |seconds:1|target:100|time:20261001-0000"],"sideNames":["template:Yes","template:No"]}}}"#
    );
    assert_eq!(plan.back, Back::Settled(1209));
    assert_eq!(plan.what, "settle outcome 1209 to 1 on out");
    let quarter = on(&deployer(), |desk| settle_outcome(desk, 1209, "0.25".parse().unwrap()));
    assert!(quarter.is_ok(), "a standalone outcome settles to any fraction");
}

/// Testnet, 2026-10-08: question 823's named outcomes were settled one at a
/// time at 0 as each team went out, the question staying live.
#[test]
fn a_question_s_leg_settles_alone_only_at_0_or_1_and_never_its_fallback() {
    let leg = on(&deployer(), |desk| settle_outcome(desk, 1474, Fraction::ZERO)).unwrap();
    assert!(leg.action.wire().contains(r#""outcome":1474,"settleFraction":"0""#));
    assert_eq!(
        on(&deployer(), |desk| settle_outcome(desk, 1474, "0.5".parse().unwrap())).unwrap_err(),
        vec![Refusal::Deploy(crate::deploy::outcomes::actions::DeployError::NotWhole(1474))]
    );
    assert_eq!(
        on(&deployer(), |desk| settle_outcome(desk, 1472, Fraction::ZERO)).unwrap_err(),
        vec![Refusal::Fallback { outcome: 1472, question: 350 }]
    );
    assert_eq!(
        on(&deployer(), |desk| settle_outcome(desk, 1473, Fraction::ZERO)).unwrap_err(),
        vec![Refusal::NotOpen { outcome: 1473 }],
        "already settled"
    );
    assert_eq!(
        on(&deployer(), |desk| settle_outcome(desk, 4242, Fraction::ZERO)).unwrap_err(),
        vec![Refusal::NotOpen { outcome: 4242 }]
    );
}

#[test]
fn a_question_settles_its_open_legs_one_winning_the_settled_one_left_out() {
    let plan = on(&deployer(), |desk| settle_question(desk, 350, 1475)).unwrap();
    assert_eq!(
        plan.action.wire(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleQuestion2":{"question":350,"outcomeSettlements":[{"outcome":1474,"settleFraction":"0","details":"","nameAndDescription":["template:sportsContestParticipant2","participant:Portugal"],"sideNames":["Yes","No"]},{"outcome":1475,"settleFraction":"1","details":"","nameAndDescription":["template:sportsContestDraw2",""],"sideNames":["Yes","No"]}],"nameAndDescription":["template:sportsContestResult","participantA:Denmark|participantB:Portugal"]}}}"#
    );
    assert_eq!(plan.back, Back::QuestionSettled(350));
    assert_eq!(
        on(&deployer(), |desk| settle_question(desk, 350, 1473)).unwrap_err(),
        vec![Refusal::NotOpen { outcome: 1473 }]
    );
    assert_eq!(on(&deployer(), |desk| settle_question(desk, 351, 1475)).unwrap_err(), vec![Refusal::NoQuestion(351)]);
}

#[test]
fn a_sub_deployer_settles_only_what_it_was_granted() {
    let seat = granted(&["settleOutcome"]);
    assert!(on(&seat, |desk| settle_outcome(desk, 1209, Fraction::ZERO)).is_ok());
    let refused = on(&seat, |desk| settle_question(desk, 350, 1474)).unwrap_err();
    assert_eq!(refused, vec![Refusal::NotGranted { venue: "out".to_owned(), operation: "settleQuestion2" }]);
    assert_eq!(said(&refused), "out has not granted this account settleQuestion2");
}

#[test]
fn grants_send_only_what_changes() {
    let user = address(SUB);
    let asked = [
        Grant { permission: Permission::SettleOutcome, user, allowed: true },
        Grant { permission: Permission::SettleQuestion, user, allowed: true },
    ];
    let plan = on(&deployer(), |desk| set_grants(desk, &asked)).unwrap();
    assert_eq!(
        plan.action.wire(),
        format!(
            r#"{{"type":"outcomeDeploy","venue":"out","operation":{{"setSubDeployers":[{{"variant":"settleQuestion","user":"{SUB}","allowed":true}}]}}}}"#
        )
    );
    assert_eq!(plan.back, Back::Granted { venue: "out".to_owned(), grants: vec![asked[1]] });
    assert_eq!(on(&deployer(), |desk| set_grants(desk, &asked[..1])).unwrap_err(), vec![Refusal::NothingChanges]);
    assert_eq!(
        on(&granted(&["settleOutcome"]), |desk| set_grants(desk, &asked)).unwrap_err(),
        vec![Refusal::NotDeployer { venue: "out".to_owned() }]
    );
}

#[test]
fn a_venue_is_activated_by_a_free_account_and_deactivated_by_its_deployer_once_empty() {
    let plan = on(&Seat::Free, |desk| activate(desk, "hip".parse().unwrap())).unwrap();
    assert_eq!(plan.action.wire(), r#"{"type":"activateOutcomeDeployer","activate":{"venueName":"hip"}}"#);
    assert!(plan.action.own_key_only());
    assert_eq!(plan.back, Back::Activated("hip".to_owned()));
    assert_eq!(
        on(&Seat::Free, |desk| activate(desk, "out".parse().unwrap())).unwrap_err(),
        vec![Refusal::Taken("out".to_owned())]
    );
    assert_eq!(
        on(&deployer(), |desk| activate(desk, "hip".parse().unwrap())).unwrap_err(),
        vec![Refusal::HasVenue { venue: "out".to_owned() }]
    );
    assert_eq!(
        on(&deployer(), deactivate).unwrap_err(),
        vec![Refusal::LiveOutcomes { venue: "out".to_owned(), live: 5 }]
    );
    let empty = OutcomeMeta { outcomes: vec![], questions: vec![], ..meta() };
    let seat = deployer();
    let plan = deactivate(&Desk { meta: &empty, templates: &[], seat: &seat, now_ms: NOW, limits: None }).unwrap();
    assert_eq!(plan.action.wire(), r#"{"type":"activateOutcomeDeployer","deactivate":null}"#);
    assert_eq!(
        on(&granted(&["settleOutcome"]), deactivate).unwrap_err(),
        vec![Refusal::NotDeployer { venue: "out".to_owned() }]
    );
}

#[test]
fn nothing_deploys_for_an_account_with_no_venue() {
    let refused = on(&Seat::Free, |desk| settle_outcome(desk, 1209, Fraction::ONE)).unwrap_err();
    assert_eq!(refused, vec![Refusal::NoVenue]);
    assert_eq!(said(&refused), "this account deploys on no venue; activate one first");
}

#[test]
fn a_full_venue_registers_nothing_more() {
    let (meta, templates) = (meta(), templates());
    let seat = deployer();
    let limits = |active| {
        Some(crate::markets::outcomes::template::DeployerLimits {
            daily_remaining: 400,
            active_remaining: active,
            active_used: 5,
        })
    };
    let desk = |limits| Desk { meta: &meta, templates: &templates, seat: &seat, now_ms: NOW, limits };
    let typed = filled(
        "binaryPrice",
        &[("perp", "BTC"), ("priceDescription", "m"), ("seconds", "1"), ("threshold", "1"), ("time", "20261001-0000")],
    );
    assert_eq!(
        register_outcome(&desk(limits(0)), &typed, FeeScale::ONE).unwrap_err(),
        vec![Refusal::Full { needed: 1, remaining: 0 }]
    );
    assert!(register_outcome(&desk(limits(1)), &typed, FeeScale::ONE).is_ok());
    let question = filled("sportsContestResult", &[("participantA", "A"), ("participantB", "B")]);
    let named = [filled("sportsContestDraw2", &[])];
    assert_eq!(
        register_question(&desk(limits(1)), &question, FeeScale::ONE, &named).unwrap_err(),
        vec![Refusal::Full { needed: 2, remaining: 1 }],
        "one named outcome and the fallback"
    );
}

/// The venue's own templates (testnet, 2026-10-08): the Python SDK's
/// two-keyword `sportsContestResult` hashes, but the venue's template asks
/// for eight more.
#[test]
fn the_venue_s_own_template_asks_for_every_keyword_it_declares() {
    let templates = serde_json::from_str::<Templates>(TESTNET_TEMPLATES).unwrap().0;
    let (meta, seat) = (meta(), deployer());
    let desk = Desk { meta: &meta, templates: &templates, seat: &seat, now_ms: NOW, limits: None };
    let question = filled("sportsContestResult", &[("participantB", "Spain"), ("participantA", "Brazil")]);
    let named = [filled("sportsContestParticipant", &[("participant", "Brazil")]), filled("sportsContestDraw", &[])];
    let refused = register_question(&desk, &question, "1.50".parse().unwrap(), &named).unwrap_err();
    let missing: Vec<&str> = refused
        .iter()
        .filter_map(|why| match why {
            Refusal::Value { named: None, bad } if bad.problem == Problem::Missing => Some(bad.keyword.as_str()),
            _ => None,
        })
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
    let desk = Desk { meta: &meta, templates: &mainnet, seat: &seat, now_ms: NOW, limits: None };
    let studio = filled(
        "binaryPrice",
        &[
            ("threshold", "83114"),
            ("time", "20261231-1200"),
            ("seconds", "60"),
            ("priceDescription", "BTC-USDC perp mark"),
            ("perp", "BTC"),
        ],
    );
    assert!(register_outcome(&desk, &studio, FeeScale::ONE).is_ok(), "Studio's preset fills mainnet's binaryPrice");
}

#[test]
fn the_read_back_finds_a_new_row_a_settled_one_and_the_grants() {
    let mut meta = meta();
    let me = address(ME);
    let back = Back::Listed {
        venue: "out".to_owned(),
        name: "template:binaryPrice".to_owned(),
        description: "perp:BTC".to_owned(),
        above: 1475,
    };
    assert_eq!(found(&back, &meta, me), None);
    meta.outcomes.push(row(1476, "template:binaryPrice", "perp:BTC", ["template:Yes", "template:No"]));
    assert_eq!(found(&back, &meta, me).as_deref(), Some("outcome 1476 is in outcomeMeta"));
    assert_eq!(found(&Back::Settled(1209), &meta, me), None);
    meta.outcomes.retain(|row| row.outcome != 1209);
    assert!(found(&Back::Settled(1209), &meta, me).is_some());
    let grant = Grant { permission: Permission::SettleOutcome, user: address(SUB), allowed: true };
    assert!(found(&Back::Granted { venue: "out".to_owned(), grants: vec![grant] }, &meta, me).is_some());
    let revoked = Grant { allowed: false, ..grant };
    assert_eq!(found(&Back::Granted { venue: "out".to_owned(), grants: vec![revoked] }, &meta, me), None);
    assert!(found(&Back::Activated("out".to_owned()), &meta, me).is_some());
    assert_eq!(found(&Back::Deactivated, &meta, me), None);
    assert_eq!(found(&Back::QuestionSettled(350), &meta, me), None);
    meta.questions[0].settled_named_outcomes = vec![1473, 1474, 1475];
    assert!(found(&Back::QuestionSettled(350), &meta, me).is_some());
}

#[test]
fn held_permissions_are_read_by_address() {
    let meta = meta();
    assert_eq!(held_by(&meta, "out", address(SUB)), [Permission::SettleOutcome]);
    assert!(held_by(&meta, "out", address(ME)).is_empty());
    assert!(held_by(&meta, "skew", address(SUB)).is_empty());
}
