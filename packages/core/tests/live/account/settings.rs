//! How accounts are set up, read from mainnet: subaccounts and a subaccount where it lives,
//! referrals and what each address may still send.

use df_hyperliquid_core::account::{abstraction, clearinghouse_state, rate_limit, referral, spot_state, sub_accounts};
use df_hyperliquid_core::markets::Dex;

use super::{BUILDER, FRESH, UNIFIED, UNIFIED_SUB, address};
use crate::tap::{reader, same_shape};

#[tokio::test]
#[ignore = "mainnet"]
async fn subaccounts_read_from_the_master_and_at_their_own_address() {
    let client = reader();
    let listed = sub_accounts(&client, address(UNIFIED)).await.unwrap();
    assert!(listed.iter().any(|sub| sub.sub_account_user == address(UNIFIED_SUB)));
    same_shape(&client, "account/subs_unified.json");
    assert_eq!(sub_accounts(&client, address(FRESH)).await.unwrap(), [], "null is none");
    same_shape(&client, "account/subs_none_mainnet.json");
    let sub = address(UNIFIED_SUB);
    abstraction(&client, sub).await.unwrap();
    same_shape(&client, "account/mode_sub_mainnet.json");
    spot_state(&client, sub).await.unwrap();
    same_shape(&client, "account/spot_sub_mainnet.json");
    clearinghouse_state(&client, sub, Dex::Main).await.unwrap();
    same_shape(&client, "account/perp_sub_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn referrals_read_for_a_code_s_owner_and_an_account_with_none() {
    let client = reader();
    assert_eq!(referral(&client, address(BUILDER)).await.unwrap().own_code(), Some("DFU"));
    same_shape(&client, "account/referral_builder.json");
    assert_eq!(referral(&client, address(FRESH)).await.unwrap().own_code(), None);
    same_shape(&client, "account/referral_fresh_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn rate_limits_read_for_a_spent_address_a_busy_one_and_a_fresh_one() {
    let client = reader();
    for (user, recording) in [
        (BUILDER, "account/rate_limit_builder.json"),
        (UNIFIED, "account/rate_limit_unified.json"),
        (FRESH, "account/rate_limit_fresh_mainnet.json"),
    ] {
        let limit = rate_limit(&client, address(user)).await.unwrap();
        assert!(limit.cap >= 10_000, "{user}");
        same_shape(&client, recording);
    }
}
