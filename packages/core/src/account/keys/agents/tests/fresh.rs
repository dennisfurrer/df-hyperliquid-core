//! An address is fresh only if the venue has never seen it as anything.

use super::*;
use crate::account::fixtures::{BUILDER_AGENTS, ROLE_AGENT, UNIFIED, address, read};

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
