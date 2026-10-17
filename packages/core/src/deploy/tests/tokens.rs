//! Live mainnet answers, 2026-10-08: `spotDeployState` for HLP Strategy A (nothing being deployed)
//! and `spotPairDeployAuctionStatus`.

use super::*;
use crate::trade::tests_support::{Recorded, block_on};

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

#[test]
fn the_deploy_reads_name_their_type() {
    let user: Address = "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap();
    assert_eq!(
        spot_deploy_state_request(user),
        r#"{"type":"spotDeployState","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a"}"#
    );
    assert_eq!(spot_pair_deploy_auction_request(), r#"{"type":"spotPairDeployAuctionStatus"}"#);
    let state = Recorded::answering(DEPLOY_STATE);
    assert!(block_on(spot_deploy_state(&state, user)).unwrap().states.is_empty());
    assert_eq!(state.sent(), [("/info".to_owned(), spot_deploy_state_request(user))]);
    let auction = Recorded::answering(PAIR_AUCTION);
    assert_eq!(block_on(spot_pair_deploy_auction(&auction)).unwrap().start_gas, dec("500"));
    assert_eq!(auction.sent(), [("/info".to_owned(), spot_pair_deploy_auction_request())]);
}
