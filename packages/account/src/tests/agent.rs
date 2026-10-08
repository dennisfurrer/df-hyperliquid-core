use hl_core::Network;

use super::*;
use crate::fixtures::{
    BUILDER_AGENTS, BUILDER_AGENTS_TESTNET, ROLE_AGENT, ROLE_SUB, ROLE_VAULT, UNIFIED, address, read,
};

const AGENT: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
const NONCE: u64 = 1_790_412_105_001;
/// 2026-10-08, when the fixtures were read.
const NOW: u64 = 1_791_480_940_000;
const DAY: u64 = 86_400_000;

#[test]
fn a_name_is_one_to_sixteen_characters_and_never_the_expiry_form() {
    // The builder's live agent's name is sixteen characters.
    for good in ["desk", "Outcome-3712f660", "Builder Markets", "x"] {
        assert_eq!(AgentName::new(good).map(|name| name.as_str().to_owned()), Ok(good.to_owned()), "{good}");
    }
    for bad in ["", "   ", "seventeen chars!!", "desk valid_until 1"] {
        assert_eq!(AgentName::new(bad), Err(BadAgentName(bad.to_owned())), "{bad}");
    }
}

#[test]
fn an_expiry_is_written_into_the_name_and_read_back_out_of_it() {
    let name = AgentName::new("desk").unwrap();
    let until = NOW + 30 * DAY;
    let approve = ApproveAgent::expiring(address(AGENT), &name, until, NOW, NONCE).unwrap();
    let wire = approve.wire(Network::Mainnet);
    assert!(wire.contains(&format!(r#""agentName":"desk valid_until {until}","#)), "{wire}");
    assert_eq!(approve.name(), Some(format!("desk valid_until {until}").as_str()));
    assert_eq!(split_valid_until(&format!("desk valid_until {until}")), ("desk", Some(until)));
    assert_eq!(split_valid_until("desk"), ("desk", None));
    assert_eq!(split_valid_until("desk valid_until soon"), ("desk valid_until soon", None));
}

#[test]
fn an_expiry_lies_ahead_and_within_a_hundred_and_eighty_days() {
    let name = AgentName::new("desk").unwrap();
    let agent = address(AGENT);
    assert!(ApproveAgent::expiring(agent, &name, NOW + MAX_AGENT_VALIDITY_MS, NOW, NONCE).is_ok());
    for until in [NOW, NOW - 1, NOW + MAX_AGENT_VALIDITY_MS + 1] {
        assert_eq!(
            ApproveAgent::expiring(agent, &name, until, NOW, NONCE),
            Err(BadExpiry { until, now: NOW }),
            "{until}"
        );
    }
    assert_eq!(MAX_AGENT_VALIDITY_MS, 180 * DAY);
}

#[test]
fn the_unnamed_agent_posts_a_null_name_and_signs_the_empty_one() {
    let agent = address(AGENT);
    let null = ApproveAgent::unnamed(agent, NONCE);
    let empty = ApproveAgent::new(agent, "", NONCE);
    for network in Network::ALL {
        assert!(null.wire(network).contains(r#""agentName":null,"#), "{network}");
        assert!(empty.wire(network).contains(r#""agentName":"","#), "{network}");
        assert_eq!(null.digest(network), empty.digest(network), "{network}: one approval, two spellings");
    }
    assert_eq!((null.name(), empty.name()), (None, None));
    assert_eq!(
        ApproveAgent::named(agent, &AgentName::new("desk").unwrap(), NONCE),
        ApproveAgent::new(agent, "desk", NONCE)
    );
}

#[test]
fn a_named_agent_lives_until_its_expiry() {
    let agents: Vec<Agent> = read(BUILDER_AGENTS);
    assert_eq!(agents[0].name, "Outcome-3712f660");
    assert_eq!(agents[0].valid_until, Some(1_798_728_647_281));
    assert!(agents[0].is_live(NOW));
    assert!(!agents[0].is_live(1_798_728_647_281), "it signs nothing from its expiry on");
    let testnet: Vec<Agent> = read(BUILDER_AGENTS_TESTNET);
    assert_eq!(testnet.iter().map(|agent| agent.name.as_str()).collect::<Vec<_>>(), ["Outcome", "Builder Markets"]);
    let forever = Agent { name: String::new(), address: address(AGENT), valid_until: None };
    assert!(forever.is_live(u64::MAX));
}

/// There is no revoke, and an address approved again can replay what it
/// signed before: only an address the venue has never seen is fresh.
#[test]
fn an_agent_address_is_never_used_twice() {
    let fresh = address("0x00000000000000000000000000000000000000aa");
    let listed: Vec<Agent> = read(BUILDER_AGENTS);
    assert_eq!(fresh_agent(fresh, &Role::Missing, &listed, &[]), Ok(()));
    assert_eq!(fresh_agent(listed[0].address, &Role::Missing, &listed, &[]), Err(Reused::Listed));
    assert_eq!(fresh_agent(fresh, &Role::Missing, &listed, &[fresh]), Err(Reused::Retired));
    // The unified account's live agent, as `userRole` answers for it.
    let role: Role = read(ROLE_AGENT);
    assert_eq!(role.agent_of(), Some(address(UNIFIED)));
    let seen = address("0xac325205a75ad9695c1499bff980c74539f7515a");
    assert_eq!(fresh_agent(seen, &role, &[], &[]), Err(Reused::Known(role.clone())));
    assert_eq!(fresh_agent(fresh, &Role::User, &[], &[]), Err(Reused::Known(Role::User)));
}

#[test]
fn a_role_names_the_account_behind_it() {
    let sub: Role = read(ROLE_SUB);
    assert_eq!(sub, Role::SubAccount { master: address(UNIFIED) });
    assert_eq!((sub.master(), sub.agent_of()), (Some(address(UNIFIED)), None));
    let vault: Role = read(ROLE_VAULT);
    assert_eq!((vault, Role::Vault.master()), (Role::Vault, None));
}
