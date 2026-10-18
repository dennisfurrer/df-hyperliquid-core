//! Who signs for an account, read from mainnet: what the venue knows each kind of address as, named
//! agents, an approved builder fee, and whether the builder can be paid.

use df_hyperliquid_core::account::{
    Role, builder_readiness, clearinghouse_state, extra_agents, max_builder_fee, role, spot_state,
};
use df_hyperliquid_core::markets::Dex;

use super::{BUILDER, FRESH, UNIFIED, UNIFIED_SUB, address};
use crate::tap::{reader, same_shape};

#[tokio::test]
#[ignore = "mainnet"]
async fn every_kind_of_address_reads_as_its_role() {
    let client = reader();
    let unified = address(UNIFIED);
    for (user, recording) in [
        ("0xac325205a75ad9695c1499bff980c74539f7515a", "account/role_agent.json"),
        (UNIFIED_SUB, "account/role_sub.json"),
        ("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303", "account/role_vault.json"),
        ("0x70997970c51812dc3a010c7d01b50e0d17dc79c8", "account/role_user_mainnet.json"),
        ("0xc171033d5cbff7175f29dfd3a63dda3d6f8f385e", "account/role_missing_mainnet.json"),
    ] {
        let read = role(&client, address(user)).await.unwrap();
        assert!(!matches!(read, Role::Other(_)), "{user} is {read:?}");
        same_shape(&client, recording);
    }
    assert_eq!(role(&client, address(UNIFIED_SUB)).await.unwrap().master(), Some(unified));
}

#[tokio::test]
#[ignore = "mainnet"]
async fn named_agents_read_for_an_account_with_some_and_one_with_none() {
    let client = reader();
    assert!(!extra_agents(&client, address(BUILDER)).await.unwrap().is_empty());
    same_shape(&client, "account/agents_builder.json");
    assert_eq!(extra_agents(&client, address(FRESH)).await.unwrap(), []);
    same_shape(&client, "account/agents_none_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn a_builder_s_approval_and_whether_it_can_be_paid_read() {
    let client = reader();
    let builder = address(BUILDER);
    max_builder_fee(&client, address(FRESH), builder).await.unwrap();
    same_shape(&client, "account/max_builder_fee_mainnet.json");
    clearinghouse_state(&client, builder, Dex::Main).await.unwrap();
    same_shape(&client, "account/builder_perp.json");
    spot_state(&client, builder).await.unwrap();
    same_shape(&client, "account/spot_builder.json");
    builder_readiness(&client, builder).await.unwrap();
    same_shape(&client, "account/mode_builder_mainnet.json");
}
