//! What an account holds, what each kind of market may draw on, and what it is all worth against
//! the venue's own `portfolio` figure: a unified account and a standard one, read within seconds of
//! each other on 2026-10-08 with the mids of their tokens' pairs, and a vault's long read before
//! it is closed.

use std::collections::BTreeMap;

use df_hyperliquid_core::account::holdings::VaultShare;
use df_hyperliquid_core::account::{
    self, Balance, Beside, Collateral, Pays, Prices, Source, Standing, Surface, active_asset_data, clearinghouse_state,
};
use df_hyperliquid_core::earn::vaults::user_vault_equities;
use df_hyperliquid_core::markets::mids::all_mids;
use df_hyperliquid_core::markets::{Dex, Side};
use df_hyperliquid_core::{Client, Dec, Network};

use super::{LEADER, UNIFIED, address, bodies, dec};
use crate::support::venue::Venue;
use crate::support::{clock, entry, fixture};

const USDC: u32 = 0;

/// Each dex's collateral as mainnet `allPerpMetas` lists it: USDC, but USDT0 (268) on `cash`.
fn collateral() -> Collateral {
    Collateral::new().with("cash", 268)
}

/// An account's mode, spot, main dex and `xyz`, the mids of its tokens' pairs, and `portfolio`.
fn venue(mode: &str, spot: &str, main: &str, xyz: &str, portfolio: &str) -> Venue {
    Venue::mainnet()
        .info("userAbstraction", mode)
        .info("spotClearinghouseState", spot)
        .info_where("clearinghouseState", &[("dex", "xyz")], &fixture(xyz))
        .info("clearinghouseState", main)
        .info_text("allMids", &entry("account/prices.json", "mids"))
        .info("portfolio", portfolio)
}

/// Each token at its USDC pair's mid, the pairs as `spotMeta` lists them.
async fn prices(client: &Client<Venue>) -> Prices {
    let mids = all_mids(client, Dex::Main).await.unwrap();
    let pairs: BTreeMap<String, String> = serde_json::from_str(&entry("account/prices.json", "pairs")).unwrap();
    Prices::of(&pairs, &mids.iter().map(|(pair, mid)| (pair.to_owned(), mid)).collect())
}

fn usable(standing: &Standing, prices: &Prices, surface: Surface<'_>) -> (Dec, Source) {
    let usable = standing.usable(surface, &collateral(), |coin: &str| prices.price(coin));
    (usable.amount, usable.from)
}

/// `{"type":kind,"user":user}` and `rest`, as the reads post them.
fn read(kind: &str, user: &str, rest: &str) -> String {
    format!(r#"{{"type":"{kind}","user":"{user}"{rest}}}"#)
}

#[tokio::test]
async fn a_unified_account_draws_every_market_on_spot_and_counts_its_perps_once() {
    let venue = venue(
        "account/mode_unified_mainnet.json",
        "account/spot_unified.json",
        "account/perp_unified.json",
        "account/perp_unified_xyz.json",
        "account/portfolio_unified.json",
    );
    let client = Client::reader(venue, Network::Mainnet, clock);
    let user = address(UNIFIED);

    let standing = account::standing(&client, user, &[Dex::Main, Dex::Hip3("xyz")]).await.unwrap();
    let prices = prices(&client).await;
    let venue_says = account::portfolio(&client, user).await.unwrap().value().unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            read("userAbstraction", UNIFIED, ""),
            read("spotClearinghouseState", UNIFIED, ""),
            read("clearinghouseState", UNIFIED, ""),
            read("clearinghouseState", UNIFIED, r#","dex":"xyz""#),
            r#"{"type":"allMids"}"#.to_owned(),
            read("portfolio", UNIFIED, ""),
        ]
    );
    // Spot USDC less its hold: the main dex's own `withdrawable` (7046.148664) is no balance of its own.
    let free = dec("736222.0525745799");
    assert_eq!(
        standing.balances(&collateral()),
        [Balance { token: USDC, total: dec("1279260.79987858"), available: free, pays: Pays::Any }]
    );
    for surface in
        [Surface::Perps { dex: "" }, Surface::Perps { dex: "xyz" }, Surface::Spot { quote: USDC }, Surface::Outcomes]
    {
        assert_eq!(usable(&standing, &prices, surface), (free, Source::Unified), "{surface:?}");
    }
    assert_eq!(usable(&standing, &prices, Surface::Perps { dex: "cash" }).0, Dec::ZERO, "no USDT0 held");
    let worth = standing.worth(&prices, &Beside::default()).unwrap();
    assert_eq!((worth.perps, worth.margin), (dec("549463.356168"), dec("480263.227504")));
    assert_eq!(worth.spot, dec("729799.2062150256144"), "spot at mids, the perps inside it taken out");
    let total = worth.total().unwrap();
    assert!(venue_says.checked_sub(total).unwrap().abs() < dec("0.001"), "{total} against the venue's {venue_says}");
}

#[tokio::test]
async fn a_standard_account_draws_each_ledger_on_its_own_and_counts_its_vaults_beside() {
    let venue = venue(
        "account/mode_standard_mainnet.json",
        "account/spot_standard.json",
        "account/perp_standard.json",
        "account/perp_standard_xyz.json",
        "account/portfolio_standard.json",
    )
    .info("userVaultEquities", "account/vaults_standard.json");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let user = address(LEADER);

    let standing = account::standing(&client, user, &[Dex::Main, Dex::Hip3("xyz")]).await.unwrap();
    let prices = prices(&client).await;
    let vaults = user_vault_equities(&client, user).await.unwrap();
    let venue_says = account::portfolio(&client, user).await.unwrap().value().unwrap();

    assert_eq!(
        client.http().read_kinds(),
        [
            "userAbstraction",
            "spotClearinghouseState",
            "clearinghouseState",
            "clearinghouseState",
            "allMids",
            "userVaultEquities",
            "portfolio"
        ]
    );
    assert_eq!(bodies(client.http())[5], read("userVaultEquities", LEADER, ""));
    let perps = |dex: &str| Pays::Perps { dex: dex.to_owned() };
    assert_eq!(
        standing.balances(&collateral()),
        [
            Balance { token: USDC, total: dec("463.943191"), available: dec("463.943191"), pays: perps("") },
            Balance { token: USDC, total: Dec::ZERO, available: Dec::ZERO, pays: perps("xyz") },
            Balance { token: USDC, total: dec("246.63995144"), available: dec("246.63995144"), pays: Pays::Spot },
        ]
    );
    let on = |dex: &str| Source::Perps { dex: dex.to_owned() };
    assert_eq!(usable(&standing, &prices, Surface::Perps { dex: "" }), (dec("463.943191"), on("")));
    assert_eq!(usable(&standing, &prices, Surface::Perps { dex: "xyz" }), (Dec::ZERO, on("xyz")));
    assert_eq!(usable(&standing, &prices, Surface::Outcomes), (dec("246.63995144"), Source::Spot));
    let balances = standing.balances(&collateral());
    assert_eq!(account::payer(&balances, Surface::Perps { dex: "flx" }, &collateral()), None, "no USDH read");

    let vaults = vaults.iter().map(|held| VaultShare { vault: held.vault_address, equity: held.equity }).collect();
    let worth = standing.worth(&prices, &Beside { vaults, ..Beside::default() }).unwrap();
    assert_eq!((worth.perps, worth.spot), (dec("463.943191"), dec("574.7059086470572")));
    assert_eq!(worth.vaults, dec("366712.374359"));
    let total = worth.total().unwrap();
    assert!(venue_says.checked_sub(total).unwrap().abs() < dec("0.1"), "{total} against the venue's {venue_says}");
}

/// HLP Strategy A's clearinghouse and its `activeAssetData` on BTC, mainnet, 2026-10-08.
#[tokio::test]
async fn a_long_is_read_with_what_a_sell_closing_it_may_take() {
    let vault = "0x010461c14e146ac35fe42271bdc1134ee31c703a";
    let venue = Venue::mainnet()
        .info("clearinghouseState", "perps/clearinghouse_hlp_a_mainnet.json")
        .info("activeAssetData", "perps/active_asset_data_btc_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let state = clearinghouse_state(&client, address(vault), Dex::Main).await.unwrap();
    let btc = state.position("BTC").unwrap();
    let active = active_asset_data(&client, address(vault), &btc.coin).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [read("clearinghouseState", vault, ""), read("activeAssetData", vault, r#","coin":"BTC""#)]
    );
    assert_eq!(state.positions().count(), 4);
    assert_eq!((btc.closing_side(), btc.size()), (Some(Side::Sell), dec("0.75104")));
    assert_eq!(active.leverage, btc.leverage, "the leverage in force is the position's");
    assert!(active.max_size(Side::Sell) > btc.size());
    assert!(active.available(Side::Sell) > active.available(Side::Buy), "a sell closes the long first");
}
