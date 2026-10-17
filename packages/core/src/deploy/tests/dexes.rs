//! `perpDeployAuctionStatus` as mainnet answered it, the round ended.

use super::*;
use crate::trade::tests_support::{Recorded, block_on};
use crate::utils::Dec;

const AUCTION_MAINNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_deploy_auction_mainnet.json"));

#[test]
fn the_perp_deploy_auction_is_read_by_its_type() {
    assert_eq!(perp_deploy_auction_request(), r#"{"type":"perpDeployAuctionStatus"}"#);
    let transport = Recorded::answering(AUCTION_MAINNET);
    let ended = block_on(perp_deploy_auction(&transport)).unwrap();
    assert_eq!(ended.end_gas, Some(Dec::parse("541.69363865").unwrap()));
    assert_eq!(transport.sent(), [("/info".to_owned(), perp_deploy_auction_request())]);
}
