use hl_core::{BuilderFee, Network};

use super::*;
use crate::{
    fixtures::{
        BUILDER, BUILDER_AGENTS, BUILDER_AGENTS_TESTNET, BUILDER_PERP, BUILDER_PERP_TESTNET, BUILDER_SPOT, UNIFIED,
        UNIFIED_SUB, address, dec, read,
    },
    mode::AbstractionMode,
};

/// 2026-10-08, when the fixtures were read.
const NOW: u64 = 1_791_480_940_000;
/// The testnet agent `Outcome`, valid until 1793934337062.
const OUTCOME_AGENT: &str = "0xcdcd4b7cc752c65a7e08fec61937982adab7672d";

const FRESH_PERP: &str = r#"{"marginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"0.0","assetPositions":[],"time":1791305116226}"#;
const TESTNET_SPOT: &str =
    r#"{"balances":[{"coin":"USDC","token":0,"total":"2500.00975","hold":"0.0","entryNtl":"0.0"}]}"#;

fn wants_everything() -> Wants {
    Wants {
        agent: Some(address(OUTCOME_AGENT)),
        builder: Some((address(BUILDER), BuilderFee::from_tenths(880))),
        mode: Some(AbstractionMode::Disabled),
    }
}

fn statuses(steps: &[Step]) -> Vec<(&'static str, Status)> {
    steps.iter().map(|step| (step.kind.id(), step.status.clone())).collect()
}

#[test]
fn a_fresh_account_deposits_before_anything_else() {
    let (perp, spot, agents): (PerpState, SpotState, Vec<Agent>) =
        (read(FRESH_PERP), read(r#"{"balances":[]}"#), vec![]);
    let seen = Seen {
        perp: &perp,
        spot: &spot,
        mode: &Abstraction::Default,
        agents: &agents,
        agent_role: None,
        approved: None,
    };
    let owner = AccountRef::own(address(BUILDER));
    let steps = steps(&owner, Network::Mainnet, &seen, &wants_everything(), NOW);
    let blocked = Status::Blocked(Blocked::DepositFirst);
    assert_eq!(
        statuses(&steps),
        [
            ("deposit", Status::Needed),
            ("approve-agent", blocked.clone()),
            ("builder-fee", blocked.clone()),
            ("account-mode", blocked)
        ]
    );
    assert_eq!(steps[0].kind, StepKind::Deposit { chain_id: 42_161, min: dec("5") }, "USDC to the bridge on Arbitrum");
    assert_eq!(left(&steps).count(), 4);
}

/// hip4-tui's builder on testnet, live: funded, standard, its agent
/// `Outcome` approved until 1793934337062, and 0.88% approved.
#[test]
fn the_live_testnet_builder_has_nothing_left_to_do() {
    let (perp, spot, agents): (PerpState, SpotState, Vec<Agent>) =
        (read(BUILDER_PERP_TESTNET), read(TESTNET_SPOT), read(BUILDER_AGENTS_TESTNET));
    let fee = Some(BuilderFee::from_tenths(880));
    let seen = Seen {
        perp: &perp,
        spot: &spot,
        mode: &Abstraction::Disabled,
        agents: &agents,
        agent_role: None,
        approved: fee,
    };
    let owner = AccountRef::own(address(BUILDER));
    let now = steps(&owner, Network::Testnet, &seen, &wants_everything(), NOW);
    assert!(left(&now).next().is_none(), "{now:?}");
    assert_eq!(now[0].kind, StepKind::Deposit { chain_id: 421_614, min: dec("5") }, "Arbitrum Sepolia on testnet");
    let later = steps(&owner, Network::Testnet, &seen, &wants_everything(), 1_793_934_337_062);
    assert_eq!(statuses(&later)[1], ("approve-agent", Status::Needed), "an agent past its expiry signs nothing");
}

#[test]
fn what_is_left_follows_what_the_account_says_now() {
    let (perp, spot, agents): (PerpState, SpotState, Vec<Agent>) =
        (read(BUILDER_PERP), read(BUILDER_SPOT), read(BUILDER_AGENTS));
    let owner = AccountRef::own(address(BUILDER));
    let low = Some(BuilderFee::from_tenths(100));
    let unified = Abstraction::UnifiedAccount;
    let seen = Seen { perp: &perp, spot: &spot, mode: &unified, agents: &agents, agent_role: None, approved: low };
    let steps = steps(&owner, Network::Mainnet, &seen, &wants_everything(), NOW);
    assert_eq!(
        statuses(&steps),
        [
            ("deposit", Status::Done),
            ("approve-agent", Status::Needed),
            ("builder-fee", Status::Needed),
            (
                "account-mode",
                Status::Blocked(Blocked::Refused("the venue refuses a unified account back to standard".to_owned()))
            ),
        ],
        "the testnet agent is not this network's, 0.1% is under 0.88%, and unified stays unified"
    );
    assert_eq!(
        steps[2].kind,
        StepKind::ApproveBuilderFee { builder: address(BUILDER), fee: BuilderFee::from_tenths(880) }
    );
    assert_eq!(steps[2].kind.action(), Some(ActionKind::ApproveBuilderFee));
}

#[test]
fn nothing_app_specific_is_asked_for_unless_passed_in() {
    let (perp, spot, agents): (PerpState, SpotState, Vec<Agent>) =
        (read(BUILDER_PERP), read(BUILDER_SPOT), read(BUILDER_AGENTS));
    let seen = Seen {
        perp: &perp,
        spot: &spot,
        mode: &Abstraction::Disabled,
        agents: &agents,
        agent_role: None,
        approved: None,
    };
    let owner = AccountRef::own(address(BUILDER));
    let none = Wants::default();
    assert_eq!(statuses(&steps(&owner, Network::Mainnet, &seen, &none, NOW)), [("deposit", Status::Done)]);
    let listed = Wants { agent: Some(agents[0].address), ..Wants::default() };
    assert_eq!(statuses(&steps(&owner, Network::Mainnet, &seen, &listed, NOW))[1], ("approve-agent", Status::Done));
}

/// The unnamed agent is not in `extraAgents`; `userRole` says whose it is.
#[test]
fn the_unnamed_agent_is_known_by_its_role() {
    let (perp, spot): (PerpState, SpotState) = (read(BUILDER_PERP), read(BUILDER_SPOT));
    let owner = AccountRef::own(address(BUILDER));
    let wants = Wants { agent: Some(address(OUTCOME_AGENT)), ..Wants::default() };
    for (role, status) in [
        (Role::Agent { user: address(BUILDER) }, Status::Done),
        (Role::Agent { user: address(UNIFIED) }, Status::Needed),
        (Role::Missing, Status::Needed),
    ] {
        let seen = Seen {
            perp: &perp,
            spot: &spot,
            mode: &Abstraction::Disabled,
            agents: &[],
            agent_role: Some(&role),
            approved: None,
        };
        assert_eq!(
            statuses(&steps(&owner, Network::Mainnet, &seen, &wants, NOW))[1],
            ("approve-agent", status),
            "{role:?}"
        );
    }
}

/// A subaccount is funded by its master, traded by its master's agent, and
/// approves no builder fee of its own.
#[test]
fn a_subaccount_is_funded_by_its_master_and_approves_nothing() {
    let (perp, spot): (PerpState, SpotState) = (read(FRESH_PERP), read(r#"{"balances":[]}"#));
    let role = Role::Agent { user: address(UNIFIED) };
    let seen = Seen {
        perp: &perp,
        spot: &spot,
        mode: &Abstraction::Default,
        agents: &[],
        agent_role: Some(&role),
        approved: None,
    };
    let sub = AccountRef::sub(address(UNIFIED), address(UNIFIED_SUB));
    let steps = steps(&sub, Network::Mainnet, &seen, &wants_everything(), NOW);
    assert_eq!(
        statuses(&steps),
        [
            ("deposit", Status::Needed),
            ("approve-agent", Status::Done),
            ("account-mode", Status::Blocked(Blocked::DepositFirst)),
        ],
        "the master's agent signs for it already"
    );
    assert_eq!(steps[0].kind, StepKind::FundFromMaster);
    assert_eq!(steps[0].kind.action(), Some(ActionKind::SubAccountTransfer));
    assert_eq!(steps[1].kind.action(), Some(ActionKind::ApproveAgent));
    assert_eq!(
        steps[2].kind.action(),
        Some(ActionKind::UserSetAbstraction),
        "a subaccount's user, signed by its master"
    );
}

#[test]
fn an_account_has_deposited_once_it_holds_anything() {
    let (fresh, empty): (PerpState, SpotState) = (read(FRESH_PERP), read(r#"{"balances":[]}"#));
    assert!(!deposited(&fresh, &empty));
    assert!(deposited(&read(BUILDER_PERP), &empty), "0.003569 on the main dex");
    assert!(deposited(&fresh, &read(TESTNET_SPOT)), "USDC in spot");
}
