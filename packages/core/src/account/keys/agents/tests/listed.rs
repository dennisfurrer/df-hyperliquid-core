//! An account's named agents as `extraAgents` answers them.

use super::*;
use crate::account::fixtures::{BUILDER_AGENTS, BUILDER_AGENTS_TESTNET, address, read};

const AGENT: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
/// 2026-10-08, when the fixtures were read.
const NOW: u64 = 1_791_480_940_000;

#[test]
fn a_named_agent_lives_until_its_expiry() {
    let agents: Vec<Agent> = read(BUILDER_AGENTS);
    assert_eq!(agents[0].name, "Desk-3712f660");
    assert_eq!(agents[0].valid_until, Some(1_798_728_647_281));
    assert!(agents[0].is_live(NOW));
    assert!(!agents[0].is_live(1_798_728_647_281), "it signs nothing from its expiry on");
    let testnet: Vec<Agent> = read(BUILDER_AGENTS_TESTNET);
    assert_eq!(testnet.iter().map(|agent| agent.name.as_str()).collect::<Vec<_>>(), ["Desk", "Builder Markets"]);
    let forever = Agent { name: String::new(), address: address(AGENT), valid_until: None };
    assert!(forever.is_live(u64::MAX));
}
