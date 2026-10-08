//! Token details, deploy state and testnet's meta, against recorded answers:
//!
//! - `token_details_hype_mainnet.json`: `tokenDetails` for HYPE, live,
//!   mainnet, 2026-10-08, its 94,023 genesis balances cut to two;
//! - `spot_deploy_state_mainnet.json`: `spotDeployState` for an account with
//!   nothing being deployed (HLP Strategy A), live, mainnet, 2026-10-08;
//! - `spot_pair_deploy_auction_mainnet.json`: `spotPairDeployAuctionStatus`,
//!   live, mainnet, 2026-10-08;
//! - `spot_meta_testnet.json`: `spotMeta`, live, testnet, 2026-10-08, its
//!   1,667 tokens cut to USDC, PURR, HYPE and RUBT (the first token whose
//!   index is not its place in the list, 1540 at 1536), and its pairs to
//!   those tokens' with USDC.

use hl_core::{Address, Asset, Network, TESTNET_USDC};

use crate::{
    deploy::{
        GasAuction, SpotDeployState, spot_deploy_state, spot_deploy_state_request, spot_pair_deploy_auction,
        spot_pair_deploy_auction_request,
    },
    details::{TokenDetails, token_details, token_details_request},
    meta::SpotMeta,
    support::{Recorded, block_on, dec},
};

const DETAILS: &str = include_str!("fixtures/token_details_hype_mainnet.json");
const DEPLOY_STATE: &str = include_str!("fixtures/spot_deploy_state_mainnet.json");
const PAIR_AUCTION: &str = include_str!("fixtures/spot_pair_deploy_auction_mainnet.json");
const TESTNET: &str = include_str!("fixtures/spot_meta_testnet.json");

fn testnet() -> SpotMeta {
    serde_json::from_str(TESTNET).unwrap()
}

#[test]
fn testnet_names_its_own_tokens_and_pairs_and_every_lookup_reads_them() {
    let meta = testnet();
    let hype = meta.token_named("HYPE").unwrap();
    assert_eq!((hype.index, hype.sz_decimals, hype.wei_decimals), (1105, 2, 8));
    assert_eq!(hype.token_id, "0x7317beb7cceed72ef0b346074cc8e7ab");
    assert_eq!(meta.token(1105), Some(hype));
    assert_eq!(meta.token(150), None, "mainnet's HYPE index is nothing here");
    let market = meta.market_of("HYPE", "USDC").unwrap();
    assert_eq!((market.pair.name.as_str(), market.pair.index), ("@1035", 1035));
    assert_eq!(market.asset(), Asset::spot(1035));
    assert_eq!(meta.market_named("@1035"), Some(market));
    assert_eq!(meta.market_at(Asset::from_id(11_035)), Some(market));
    let usdc = meta.usdc().unwrap();
    assert_eq!(usdc.wire(), TESTNET_USDC.wire, "agrees with hl-core's testnet USDC");
    assert_eq!(usdc.wire(), Network::Testnet.usdc().wire);
    assert_eq!(meta.token_by_id("0x7317BEB7CCEED72EF0B346074CC8E7AB"), Some(hype));
}

/// Token indexes are the venue's, not places in the list: testnet's run from
/// 0 to 1535 in order, then skip.
#[test]
fn a_token_is_found_by_its_index_not_its_place() {
    let meta = testnet();
    let rubt = meta.token(1540).unwrap();
    assert_eq!(rubt.name, "RUBT");
    assert_eq!(meta.tokens.iter().position(|token| token.index == 1540), Some(3));
    assert_eq!(meta.token(3), None);
    assert_eq!(rubt.deployer_trading_fee_share, Some(dec("1")));
}

#[test]
fn token_details_read_supply_prices_and_genesis() {
    let details: TokenDetails = serde_json::from_str(DETAILS).unwrap();
    assert_eq!(details.name, "HYPE");
    assert_eq!(details.max_supply, dec("1000000000"));
    assert_eq!(details.total_supply, dec("998874721.3441244364"));
    assert_eq!(details.circulating_supply, dec("302175535.2045443654"));
    assert_eq!((details.sz_decimals, details.wei_decimals), (2, 8), "as spotMeta lists HYPE");
    assert_eq!(
        (details.mid_px, details.mark_px, details.prev_day_px),
        (Some(dec("83.4055")), Some(dec("83.406")), Some(dec("87.84")))
    );
    assert_eq!(details.deployer, None);
    assert_eq!(details.deploy_gas, Some(dec("0")));
    assert_eq!(details.deploy_time.as_deref(), Some("2024-11-29T06:45:52.532"));
    assert_eq!(details.seeded_usdc, dec("0"));
    assert_eq!(details.future_emissions, dec("410921943.1159754992"));
    let genesis = details.genesis.as_ref().unwrap();
    assert_eq!(
        genesis.user_balances[0],
        ("0x000000000077cdff30a1b5d7c12f3587f921e519".parse::<Address>().unwrap(), dec("1819.26"))
    );
    assert!(genesis.existing_token_balances.is_empty() && genesis.blacklist_users.is_empty());
    assert_eq!(details.non_circulating_user_balances.len(), 4);
    assert_eq!(
        details.non_circulating_user_balances[3],
        ("0x000000000000000000000000000000000000dead".parse().unwrap(), dec("2.71671343"))
    );
}

#[test]
fn token_details_are_asked_by_token_id() {
    assert_eq!(
        token_details_request("0x0d01dc56dcaaca66ad901c959b4011ec"),
        r#"{"type":"tokenDetails","tokenId":"0x0d01dc56dcaaca66ad901c959b4011ec"}"#
    );
    let transport = Recorded::new(&[("tokenDetails", DETAILS)]);
    let details = block_on(token_details(&transport, "0x0d01dc56dcaaca66ad901c959b4011ec")).unwrap();
    assert_eq!(details.name, "HYPE");
}

/// The docs (and hip4-tui's DEPLOYS.md) give the deploy auctions 31 hours.
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
    let transport = Recorded::new(&[("spotDeployState", DEPLOY_STATE), ("spotPairDeployAuctionStatus", PAIR_AUCTION)]);
    assert!(block_on(spot_deploy_state(&transport, user)).unwrap().states.is_empty());
    assert_eq!(block_on(spot_pair_deploy_auction(&transport)).unwrap().start_gas, dec("500"));
    assert_eq!(transport.sent().len(), 2);
}
