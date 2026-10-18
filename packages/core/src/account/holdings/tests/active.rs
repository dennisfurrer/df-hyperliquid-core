//! `activeAssetData` as the venue writes it: HLP Strategy A on BTC
//! (`active_asset_data_btc_mainnet.json`), a minute after its `clearinghouseState`. The read runs
//! in the mocked journeys.

use super::*;
use crate::account::ClearinghouseState;
use crate::utils::{Address, Dec};

const HLP_A: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_hlp_a_mainnet.json"));
const ACTIVE_BTC: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/active_asset_data_btc_mainnet.json"));

fn vault() -> Address {
    "0x010461C14e146ac35Fe42271BDC1134EE31C703a".parse().unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// The leverage `activeAssetData` reports in force is the one the position in
/// `clearinghouseState` carries: two reads that agree. Long BTC, the vault may sell more than it
/// may buy: a sell closes the long first, which frees its margin.
#[test]
fn active_asset_data_says_the_leverage_in_force_and_what_each_side_may_trade() {
    let active: ActiveAssetData = serde_json::from_str(ACTIVE_BTC).unwrap();
    assert_eq!((active.user, active.coin.as_str(), active.mark_px), (vault(), "BTC", dec("80674")));
    let hlp: ClearinghouseState = serde_json::from_str(HLP_A).unwrap();
    assert_eq!(active.leverage, hlp.position("BTC").unwrap().leverage);
    assert_eq!((active.max_size(Side::Buy), active.max_size(Side::Sell)), (dec("740.51701"), dec("742.01909")));
    assert_eq!(
        (active.available(Side::Buy), active.available(Side::Sell)),
        (dec("2987023.4632370002"), dec("2993082.4033329999"))
    );
    assert!(active.available(Side::Sell) > active.available(Side::Buy));
}
