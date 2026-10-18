//! What accounts hold, read from mainnet: a unified account and a standard one, each dex, spot and
//! the venue's own history of their worth, and what an order on a vault's BTC may take.

use df_hyperliquid_core::account::{
    Abstraction, Keeping, abstraction, active_asset_data, clearinghouse_state, portfolio, spot_state, standing,
};
use df_hyperliquid_core::markets::Dex;

use super::{LEADER, UNIFIED, address};
use crate::tap::{reader, same_shape};

#[tokio::test]
#[ignore = "mainnet"]
async fn a_unified_account_reads_its_mode_spot_every_dex_and_portfolio() {
    let client = reader();
    let user = address(UNIFIED);
    assert_eq!(abstraction(&client, user).await.unwrap(), Abstraction::UnifiedAccount, "never back to standard");
    same_shape(&client, "account/mode_unified_mainnet.json");
    spot_state(&client, user).await.unwrap();
    same_shape(&client, "account/spot_unified.json");
    clearinghouse_state(&client, user, Dex::Main).await.unwrap();
    same_shape(&client, "account/perp_unified.json");
    clearinghouse_state(&client, user, Dex::Hip3("xyz")).await.unwrap();
    same_shape(&client, "account/perp_unified_xyz.json");
    let held = standing(&client, user, &[Dex::Main]).await.unwrap();
    assert_eq!(held.keeping(), Keeping::Unified);
    same_shape(&client, "account/perp_unified.json");
    assert!(portfolio(&client, user).await.unwrap().value().is_some());
    same_shape(&client, "account/portfolio_unified.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn a_standard_account_reads_its_mode_spot_every_dex_and_portfolio() {
    let client = reader();
    let user = address(LEADER);
    assert!(abstraction(&client, user).await.unwrap().is_standard());
    same_shape(&client, "account/mode_standard_mainnet.json");
    spot_state(&client, user).await.unwrap();
    same_shape(&client, "account/spot_standard.json");
    clearinghouse_state(&client, user, Dex::Main).await.unwrap();
    same_shape(&client, "account/perp_standard.json");
    clearinghouse_state(&client, user, Dex::Hip3("xyz")).await.unwrap();
    same_shape(&client, "account/perp_standard_xyz.json");
    assert!(portfolio(&client, user).await.unwrap().value().is_some());
    same_shape(&client, "account/portfolio_standard.json");
}

/// HLP Strategy A, as its recording was taken.
#[tokio::test]
#[ignore = "mainnet"]
async fn a_vault_reads_what_an_order_on_btc_may_take() {
    let client = reader();
    let active =
        active_asset_data(&client, address("0x010461c14e146ac35fe42271bdc1134ee31c703a"), "BTC").await.unwrap();
    assert_eq!(active.coin, "BTC");
    same_shape(&client, "perps/active_asset_data_btc_mainnet.json");
}
