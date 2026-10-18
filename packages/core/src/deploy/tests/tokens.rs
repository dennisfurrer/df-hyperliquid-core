//! Live mainnet answers, 2026-10-08: `spotDeployState` for HLP Strategy A (nothing being deployed)
//! and `spotPairDeployAuctionStatus`. The reads through a client are in `tests/mocked/deploy.rs`.

use super::*;

const DEPLOY_STATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/spot_deploy_state_mainnet.json"));
const PAIR_AUCTION: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/spot_pair_deploy_auction_mainnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// Hyperliquid's docs give the deploy auctions 31 hours.
#[test]
fn the_deploy_auctions_read_their_gas_and_their_hours() {
    let state: SpotDeployState = serde_json::from_str(DEPLOY_STATE).unwrap();
    assert!(state.states.is_empty(), "nothing being deployed");
    let auction = &state.gas_auction;
    assert_eq!(auction.duration_seconds, 31 * 3600);
    assert_eq!((auction.start_time_seconds, auction.ends_at()), (1_791_403_200, 1_791_514_800));
    assert_eq!((auction.start_gas, auction.current_gas, auction.end_gas), (dec("500"), Some(dec("500")), None));
    assert!(auction.is_running(1_791_480_567));
    assert!(!auction.is_running(1_791_514_800), "over at its end");
    assert!(!auction.is_running(1_791_403_199), "not before its start");
    let pair: GasAuction = serde_json::from_str(PAIR_AUCTION).unwrap();
    assert_eq!(pair.duration_seconds, 31 * 3600);
    assert_eq!(pair.current_gas, Some(dec("500")));
}
