//! What `userRole` answers, read.

use super::*;
use crate::account::fixtures::{ROLE_SUB, ROLE_VAULT, UNIFIED, address, read};

#[test]
fn a_role_names_the_account_behind_it() {
    let sub: Role = read(ROLE_SUB);
    assert_eq!(sub, Role::SubAccount { master: address(UNIFIED) });
    assert_eq!((sub.master(), sub.agent_of()), (Some(address(UNIFIED)), None));
    let vault: Role = read(ROLE_VAULT);
    assert_eq!((vault, Role::Vault.master()), (Role::Vault, None));
}
