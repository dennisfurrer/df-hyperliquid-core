//! Live answers, 2026-10-08: on mainnet, `tokenDetails` for HYPE (94,023 genesis balances cut to
//! two); on testnet, `spotMeta` with its 1,667 tokens cut to USDC, PURR, HYPE and RUBT (the first
//! token whose index is not its place, 1540 at 1536) and their USDC pairs. Reads through a client
//! are in `tests/mocked/markets`.

use crate::client::{Network, TESTNET_USDC};
use crate::markets::spot::details::TokenDetails;
use crate::markets::spot::meta::SpotMeta;
use crate::utils::{Address, Asset, Dec};

const DETAILS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/token_details_hype_mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/spot_meta_testnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// Token indexes are the venue's, not places in the list: testnet's run from 0 to 1535 in order,
/// then skip.
#[test]
fn testnet_names_its_own_tokens_and_pairs_by_index_not_place() {
    let meta: SpotMeta = serde_json::from_str(TESTNET).unwrap();
    let hype = meta.token_named("HYPE").unwrap();
    assert_eq!((hype.index, hype.sz_decimals, hype.wei_decimals), (1105, 2, 8));
    assert_eq!((meta.token(1105), meta.token_by_id("0x7317BEB7CCEED72EF0B346074CC8E7AB")), (Some(hype), Some(hype)));
    assert_eq!(meta.token(150), None, "mainnet's HYPE index is nothing here");
    let market = meta.market_of("HYPE", "USDC").unwrap();
    assert_eq!((market.pair.name.as_str(), market.asset()), ("@1035", Asset::spot(1035)));
    assert_eq!((meta.market_named("@1035"), meta.market_at(Asset::from_id(11_035))), (Some(market), Some(market)));
    let usdc = meta.usdc().unwrap();
    assert_eq!(usdc.wire(), TESTNET_USDC.wire, "agrees with the testnet USDC constant");
    assert_eq!(usdc.wire(), Network::Testnet.usdc().wire);
    let rubt = meta.token(1540).unwrap();
    assert_eq!((rubt.name.as_str(), rubt.deployer_trading_fee_share), ("RUBT", Some(dec("1"))));
    assert_eq!((meta.tokens.iter().position(|token| token.index == 1540), meta.token(3)), (Some(3), None));
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
