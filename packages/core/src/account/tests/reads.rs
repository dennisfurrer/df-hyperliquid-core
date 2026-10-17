//! Live answers, mainnet, 2026-10-06, some trimmed as marked.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::account::agent::{Agent, Role, extra_agents};
use crate::account::builder::{max_builder_fee, max_builder_fee_request};
use crate::account::referral::Referral;
use crate::account::sub_account::{SubAccount, sub_accounts};
use crate::client::{Transport, TransportError};
use crate::trade::builder_fee::BuilderFee;
use crate::utils::{Address, Dec};

/// HLP's leader's named agents, trimmed to two of six.
const AGENTS: &str = r#"[{"name":"a3","address":"0xeeb5fb0d32a403733ebbf6e1c694f9253b37cf4c","validUntil":1793721457998},{"name":"a2","address":"0x208c65ee65efc668e1d9960ffa6fe57c7e633079","validUntil":1793721404275}]"#;
const SUBS: &str = r#"[{"name":"ATLAS-SUPERVISED","subAccountUser":"0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f","master":"0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d","clearinghouseState":{"marginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"0.0","assetPositions":[],"time":1791305840519},"spotState":{"balances":[{"coin":"USDC","token":0,"total":"0.0","hold":"0.0","entryNtl":"0.0"}]}}]"#;
/// A code's owner, its referral rows and reward history trimmed to one each.
const REFERRER: &str = r#"{"referredBy":null,"cumVlm":"7049.82","unclaimedRewards":"1.39565981","claimedRewards":"1.799226","builderRewards":"0.0","referrerState":{"stage":"ready","data":{"code":"TESTCODE","nReferrals":3,"referralStates":[{"cumVlm":"2547126.0","cumRewardedFeesSinceReferred":"15.83428","cumFeesRewardedToReferrer":"0.874721","timeJoined":1775271678880,"user":"0xb6a6123aca000befa2b18fb65e7ec489ee1dde92","tokenToState":[[0,{"cumVlm":"2547126.0","cumRewardedFeesSinceReferred":"15.83428","cumFeesRewardedToReferrer":"0.874721"}]]}]}},"rewardHistory":[{"earned":"0.24564","vlm":"3541.88913","referralVlm":"0.0","time":1686181525632}],"tokenToState":[[0,{"cumVlm":"7049.82","unclaimedRewards":"1.39565981","claimedRewards":"1.799226","builderRewards":"0.0"}]]}"#;
const REFERRED: &str = r#"{"referredBy":{"referrer":"0x677d831aef5328190852e24f13c46cac05f984e7","code":"TESTCODE"},"cumVlm":"2547126.0","unclaimedRewards":"0.0","claimedRewards":"0.0","builderRewards":"0.0","referrerState":{"stage":"needToCreateCode"},"rewardHistory":[],"tokenToState":[[0,{"cumVlm":"2547126.0","unclaimedRewards":"0.0","claimedRewards":"0.0","builderRewards":"0.0"}]]}"#;
const UNTRADED: &str = r#"{"referredBy":null,"cumVlm":"0.0","unclaimedRewards":"0.0","claimedRewards":"0.0","builderRewards":"0.0","referrerState":{"stage":"needToTrade","data":{"required":"10000.0"}},"rewardHistory":[],"tokenToState":[]}"#;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn named_agents_read_with_when_each_expires() {
    let agents: Vec<Agent> = serde_json::from_str(AGENTS).unwrap();
    assert_eq!(agents[0].name, "a3");
    assert_eq!(agents[0].address, address("0xeeb5fb0d32a403733ebbf6e1c694f9253b37cf4c"));
    assert_eq!(agents[0].valid_until, Some(1_793_721_457_998));
}

#[test]
fn every_role_reads_and_an_agent_names_its_master() {
    for (said, role) in [
        (r#"{"role":"user"}"#, Role::User),
        (r#"{"role":"missing"}"#, Role::Missing),
        (r#"{"role":"vault"}"#, Role::Vault),
        (
            r#"{"role":"agent","data":{"user":"0x677d831aef5328190852e24f13c46cac05f984e7"}}"#,
            Role::Agent { user: address("0x677d831aef5328190852e24f13c46cac05f984e7") },
        ),
        (
            r#"{"role":"subAccount","data":{"master":"0xbaff87940cb148c5e2529c0a5944b0a3386fb015"}}"#,
            Role::SubAccount { master: address("0xbaff87940cb148c5e2529c0a5944b0a3386fb015") },
        ),
        (r#"{"role":"marketMaker"}"#, Role::Other("marketMaker".to_owned())),
        (r#"{"role":"agent","data":{"user":"not an address"}}"#, Role::Other("agent".to_owned())),
        (r#"{"role":"agent"}"#, Role::Other("agent".to_owned())),
    ] {
        assert_eq!(serde_json::from_str::<Role>(said).unwrap(), role, "{said}");
    }
}

#[test]
fn a_subaccount_reads_with_its_own_states() {
    let subs: Vec<SubAccount> = serde_json::from_str(SUBS).unwrap();
    let desk = &subs[0];
    assert_eq!(desk.name, "ATLAS-SUPERVISED");
    assert_eq!(desk.master, address("0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d"));
    assert_eq!(desk.clearinghouse_state.as_ref().map(|perp| perp.withdrawable), Some(Dec::ZERO));
    assert_eq!(desk.spot_state.as_ref().and_then(|spot| spot.usdc()).map(|usdc| usdc.total), Some(Dec::ZERO));
}

#[test]
fn referral_says_who_referred_whom_and_the_account_s_own_code() {
    let owner: Referral = serde_json::from_str(REFERRER).unwrap();
    assert_eq!((owner.own_code(), owner.referred_code()), (Some("TESTCODE"), None));
    assert_eq!(owner.referrer_state.data.as_ref().and_then(|data| data.n_referrals), Some(3));
    assert_eq!(owner.unclaimed_rewards, Dec::parse("1.39565981").unwrap());
    let referred: Referral = serde_json::from_str(REFERRED).unwrap();
    assert_eq!(referred.referred_code(), Some("TESTCODE"));
    assert_eq!(
        referred.referred_by.as_ref().map(|by| by.referrer),
        Some(address("0x677d831aef5328190852e24f13c46cac05f984e7"))
    );
    assert_eq!((referred.referrer_state.stage.as_str(), referred.own_code()), ("needToCreateCode", None));
    let untraded: Referral = serde_json::from_str(UNTRADED).unwrap();
    assert_eq!(untraded.referrer_state.data.and_then(|data| data.required), Some(Dec::from_int(10_000)));
}

/// Answers by `type`, keeping what was asked. Nothing leaves the process.
struct Recorded {
    sent: RefCell<Vec<String>>,
}

impl Transport for Recorded {
    fn post(&self, _: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        let answer = match serde_json::from_str::<serde_json::Value>(&body).unwrap()["type"].as_str() {
            Some("maxBuilderFee") => "880",
            _ => "null",
        };
        self.sent.borrow_mut().push(body);
        ready(Ok(answer.to_owned()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn none_reads_as_empty_and_a_builder_fee_as_tenths() {
    let transport = Recorded { sent: RefCell::new(Vec::new()) };
    let user = address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414");
    let builder = address("0x6967d7AcD1EC5f210E9590666498e0cC5D13a843");
    assert_eq!(block_on(extra_agents(&transport, user)).unwrap(), [], "null is no agents");
    assert_eq!(block_on(sub_accounts(&transport, user)).unwrap(), [], "null is no subaccounts");
    assert_eq!(block_on(max_builder_fee(&transport, user, builder)).unwrap(), BuilderFee::from_tenths(880));
    assert_eq!(serde_json::from_str::<Option<Vec<Agent>>>("[]").unwrap(), Some(vec![]), "the live answer for none");
    assert_eq!(
        max_builder_fee_request(user, builder),
        r#"{"type":"maxBuilderFee","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","builder":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843"}"#
    );
    assert_eq!(
        transport.sent.borrow()[0],
        r#"{"type":"extraAgents","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414"}"#
    );
}

/// A live unified account, read as a standing: its mode, spot, and each dex
/// asked for, in that order, at its own address.
#[test]
fn a_standing_reads_the_mode_spot_and_each_dex_asked_for() {
    use crate::account::fixtures::{self, PERP_UNIFIED, PERP_UNIFIED_XYZ, SPOT_UNIFIED, UNIFIED};
    use crate::account::state::Abstraction;
    let at = |kind: &str, rest: &str| format!(r#"{{"type":"{kind}","user":"{UNIFIED}"{rest}}}"#);
    let asked = [
        at("userAbstraction", ""),
        at("spotClearinghouseState", ""),
        at("clearinghouseState", ""),
        at("clearinghouseState", r#","dex":"xyz""#),
    ];
    let transport = fixtures::Recorded::new(&[
        (&asked[0], "\"unifiedAccount\""),
        (&asked[1], SPOT_UNIFIED),
        (&asked[2], PERP_UNIFIED),
        (&asked[3], PERP_UNIFIED_XYZ),
    ]);
    let standing = fixtures::block_on(crate::account::standing(&transport, address(UNIFIED), &["", "xyz"])).unwrap();
    assert_eq!(standing.mode, Some(Abstraction::UnifiedAccount));
    assert_eq!(standing.dexes.iter().map(|dex| dex.dex.as_str()).collect::<Vec<_>>(), ["", "xyz"]);
    assert_eq!(standing.dex("").map(|perp| perp.withdrawable), Some(Dec::parse("7046.148664").unwrap()));
    assert_eq!(standing.spot.as_ref().map(|spot| spot.balances.len()), Some(7));
    assert_eq!(transport.asked(), asked);
}

#[test]
fn a_role_is_read_at_the_address() {
    use crate::account::fixtures::{self, ROLE_AGENT, UNIFIED};
    let agent = address("0xac325205a75ad9695c1499bff980c74539f7515a");
    let body = r#"{"type":"userRole","user":"0xac325205a75ad9695c1499bff980c74539f7515a"}"#;
    let transport = fixtures::Recorded::new(&[(body, ROLE_AGENT)]);
    assert_eq!(fixtures::block_on(crate::account::role(&transport, agent)), Ok(Role::Agent { user: address(UNIFIED) }));
}
