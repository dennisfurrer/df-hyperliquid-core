//! What `userRole` answers, read: the recorded agent, subaccount and vault, and every role spelled
//! by hand where its data is the point.

use super::*;
use crate::account::fixtures::{ROLE_AGENT, ROLE_SUB, ROLE_VAULT, UNIFIED, address};

#[test]
fn every_role_reads_and_names_the_account_behind_it() {
    let unified = address(UNIFIED);
    for (said, role, agent_of, master) in [
        (ROLE_AGENT, Role::Agent { user: unified }, Some(unified), None),
        (ROLE_SUB, Role::SubAccount { master: unified }, None, Some(unified)),
        (ROLE_VAULT, Role::Vault, None, None),
        (r#"{"role":"user"}"#, Role::User, None, None),
        (r#"{"role":"missing"}"#, Role::Missing, None, None),
        (r#"{"role":"marketMaker"}"#, Role::Other("marketMaker".to_owned()), None, None),
        (r#"{"role":"agent","data":{"user":"not an address"}}"#, Role::Other("agent".to_owned()), None, None),
        (r#"{"role":"agent"}"#, Role::Other("agent".to_owned()), None, None),
        (r#"{"role":"subAccount","data":{}}"#, Role::Other("subAccount".to_owned()), None, None),
    ] {
        let read: Role = serde_json::from_str(said).unwrap();
        assert_eq!((read.agent_of(), read.master()), (agent_of, master), "{said}");
        assert_eq!(read, role, "{said}");
    }
}
