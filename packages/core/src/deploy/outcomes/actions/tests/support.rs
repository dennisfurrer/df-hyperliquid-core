//! What the outcome action tests share: values typed the way a caller types them, `outcomeMeta`
//! listings for a settlement to copy, and the actions of the Python SDK's script.

use crate::deploy::outcomes::actions::{
    ActivateOutcomeDeployer, FeeScale, Fraction, Grant, Instance, Keywords, OutcomeDeploy, Permission, Venue,
};
use crate::markets::outcomes::meta::{Outcome, OutcomeMeta, Question, SideSpec};
use crate::utils::Address;

pub const LEADER: &str = "0x251fd19df63a83652eea93a6a5e451aa5ce81e4c";
pub const AGENT: &str = "0xab04c8d8372d83ec939570fd2d956d0bb31aace3";

/// 2026-09-30 12:00 UTC, the clock every dated check here reads.
pub const NOW_MS: u64 = 1_790_769_600_000;

pub fn venue(name: &str) -> Venue {
    name.parse().unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn keywords(pairs: &[(&str, &str)]) -> Keywords {
    Keywords::from_pairs(pairs.iter().copied()).unwrap()
}

pub fn instance(id: &str, pairs: &[(&str, &str)]) -> Instance {
    Instance::new(id, keywords(pairs))
}

/// An outcome as `outcomeMeta` lists it, its words exactly as given.
pub fn row(outcome: u32, name: &str, description: &str, sides: [&str; 2]) -> Outcome {
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
pub fn listing(outcomes: Vec<Outcome>) -> OutcomeMeta {
    OutcomeMeta { outcomes, ..OutcomeMeta::default() }
}

/// `outcomeMeta` listing `legs` as the named outcomes of `question`, none settled, no fallback.
pub fn asked(question: u32, name: &str, description: &str, legs: Vec<Outcome>) -> OutcomeMeta {
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
pub fn python_standalone() -> OutcomeDeploy {
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

pub fn python_question() -> OutcomeDeploy {
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

pub fn python_associate() -> OutcomeDeploy {
    OutcomeDeploy::register_and_associate(
        venue("zzz"),
        182,
        instance("sportsContestParticipant", &[("participant", "Spain")]),
    )
}

pub fn python_sub_deployers() -> OutcomeDeploy {
    OutcomeDeploy::set_sub_deployers(
        venue("zzz"),
        vec![
            Grant { permission: Permission::SettleOutcome, user: address(AGENT), allowed: true },
            Grant { permission: Permission::SettleQuestion, user: address(LEADER), allowed: false },
        ],
    )
    .unwrap()
}

pub fn python_settle_outcome() -> OutcomeDeploy {
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

pub fn python_settle_question() -> OutcomeDeploy {
    OutcomeDeploy::settle_question(venue("zzz"), &chickens(), 182, 7004).unwrap()
}

pub fn python_activate() -> ActivateOutcomeDeployer {
    ActivateOutcomeDeployer::Activate(venue("zzz"))
}
