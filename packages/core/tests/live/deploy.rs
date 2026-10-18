//! Deploy reads from mainnet, each held against its recording: the spot and perp deploy auctions,
//! an account's spot deploys, and what an outcome deployer reads before registering.

use df_hyperliquid_core::deploy::{perp_deploy_auction, spot_deploy_state, spot_pair_deploy_auction};
use df_hyperliquid_core::markets::outcomes::{deployer_limits, outcome_templates};
use df_hyperliquid_core::utils::Address;

use crate::tap::{last, reader, same_shape};

/// Each deploy round lasts 31 hours.
const ROUND: u64 = 31 * 3_600;

#[tokio::test]
#[ignore = "mainnet"]
async fn every_deploy_auction_runs_31_hour_rounds() {
    let client = reader();
    let strategy: Address = "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap();
    let state = spot_deploy_state(&client, strategy).await.unwrap();
    assert_eq!(state.gas_auction.duration_seconds, ROUND);
    same_shape(&client, "spot/spot_deploy_state_mainnet.json");
    assert_eq!(spot_pair_deploy_auction(&client).await.unwrap().duration_seconds, ROUND);
    same_shape(&client, "spot/spot_pair_deploy_auction_mainnet.json");
    assert_eq!(perp_deploy_auction(&client).await.unwrap().duration_seconds, ROUND);
    same_shape(&client, "hip3/perp_deploy_auction_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn an_outcome_deployer_reads_every_template_and_what_its_venue_may_still_register() {
    let client = reader();
    let templates = outcome_templates(&client).await.unwrap();
    let listed = last(&client).as_array().map(Vec::len);
    assert_eq!(Some(templates.len()), listed, "every template reads");
    same_shape(&client, "hip4/outcome_templates_mainnet.json");
    deployer_limits(&client, "out").await.unwrap();
    same_shape(&client, "hip4/deployer_limits_out_mainnet.json");
}
