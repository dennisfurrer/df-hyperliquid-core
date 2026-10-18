//! What mainnet says about a recipient before a send.

use df_hyperliquid_core::transfer::pre_transfer_check;
use df_hyperliquid_core::utils::Address;

use crate::tap::{reader, same_shape};

#[tokio::test]
#[ignore = "mainnet"]
async fn hlp_as_a_recipient_exists_and_costs_its_leader_no_activation_fee() {
    let client = reader();
    let hlp: Address = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303".parse().unwrap();
    let leader: Address = "0x677d831aef5328190852e24f13c46cac05f984e7".parse().unwrap();
    let check = pre_transfer_check(&client, hlp, leader).await.unwrap();
    assert!(check.user_exists && !check.is_sanctioned);
    assert_eq!(check.activation_fee(), None);
    same_shape(&client, "transfer/pre_transfer_check_mainnet.json#known");
}
