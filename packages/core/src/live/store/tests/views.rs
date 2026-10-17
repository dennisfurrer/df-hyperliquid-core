use std::collections::BTreeMap;

use crate::account::ClearinghouseState;
use crate::account::worth::VaultShare;
use crate::account::{Abstraction, Beside, Source, SpotState, Surface, Usable};
use crate::client::Network;
use crate::live::store::fixtures::{
    PERP_STANDARD, PERP_STANDARD_XYZ, PERP_UNIFIED, PERP_UNIFIED_XYZ, PORTFOLIO_STANDARD, PORTFOLIO_UNIFIED, PRICES,
    SPOT_STANDARD, SPOT_UNIFIED, STANDARD, SUBS_UNIFIED, UNIFIED, UNIFIED_SUB, address, dec, dexes, frame, read,
};
use crate::live::store::{Handle, Interest, Read, Store};
use crate::markets::Mids;
use crate::markets::spot::SpotMeta;
use crate::utils::{Address, Dec};
use serde::Deserialize;

fn store() -> Store {
    Store::new(Network::Mainnet)
}

/// The recorded prices: each token's USDC pair (from `spotMeta`) and those pairs' mids (from
/// `allMids`), read straight after the accounts.
#[derive(Deserialize)]
struct Recorded {
    pairs: BTreeMap<String, String>,
    mids: BTreeMap<String, String>,
}

/// `spotMeta` as far as the recorded pairs go: each token by the index the recorded balances
/// carry, each pair by its name. Token ids and decimals are not read here and are left blank.
fn spot_meta(recorded: &Recorded) -> SpotMeta {
    let index = |name: &str| match name {
        "USDC" => 0,
        "PURR" => 1,
        "MUNCH" => 146,
        "HYPE" => 150,
        "UBTC" => 197,
        "UETH" => 221,
        "USDE" => 235,
        "USDT0" => 268,
        "USDH" => 360,
        "MAX" => 734,
        other => panic!("no index recorded for {other}"),
    };
    let token = |name: &str| serde_json::json!({"name": name, "index": index(name), "tokenId": "0x", "szDecimals": 0, "weiDecimals": 0});
    let tokens: Vec<_> = std::iter::once("USDC").chain(recorded.pairs.keys().map(String::as_str)).map(token).collect();
    let universe: Vec<_> = recorded
        .pairs
        .iter()
        .map(|(name, pair)| {
            let at: u32 = pair.strip_prefix('@').map_or(0, |at| at.parse().unwrap());
            serde_json::json!({"name": pair, "tokens": [index(name), 0], "index": at})
        })
        .collect();
    read(&serde_json::json!({"tokens": tokens, "universe": universe}).to_string())
}

/// The recorded prices into `store`: spot meta, and the first dex's mids.
fn priced(store: &mut Store) {
    let recorded: Recorded = read(PRICES);
    let meta = spot_meta(&recorded);
    let mids: Mids = read(&serde_json::to_string(&recorded.mids).unwrap());
    let _ = store.read(1, Read::SpotMeta(&meta));
    let _ = store.read(1, Read::Mids { dex: "", mids: &mids });
}

/// One account read whole: its mode, spot, and the first dex and `xyz`.
fn account(store: &mut Store, user: Address, mode: Abstraction, spot: &str, main: &str, xyz: &str) -> Handle {
    let handle = store.register(Interest::State { user }).unwrap();
    let spot: SpotState = read(spot);
    let (main, xyz): (ClearinghouseState, ClearinghouseState) = (read(main), read(xyz));
    let _ = store.read(1, Read::Dexes(&dexes()));
    let _ = store.read(1, Read::Mode { user, mode: &mode });
    let _ = store.read(1, Read::SpotState { user, state: &spot });
    let _ = store.read(1, Read::Clearinghouse { user, dex: "", state: &main });
    let _ = store.read(1, Read::Clearinghouse { user, dex: "xyz", state: &xyz });
    handle
}

fn unified(store: &mut Store) -> Handle {
    account(store, address(UNIFIED), Abstraction::UnifiedAccount, SPOT_UNIFIED, PERP_UNIFIED, PERP_UNIFIED_XYZ)
}

fn standard(store: &mut Store) -> Handle {
    account(store, address(STANDARD), Abstraction::Default, SPOT_STANDARD, PERP_STANDARD, PERP_STANDARD_XYZ)
}

/// The last point of `portfolio`'s `day`: the venue's own figure for what the account is worth.
fn venue_says(portfolio: &str) -> Dec {
    let spans: Vec<(String, serde_json::Value)> = read(portfolio);
    let day = &spans.iter().find(|(name, _)| name == "day").unwrap().1["accountValueHistory"];
    dec(day.as_array().unwrap().last().unwrap()[1].as_str().unwrap())
}

fn usable(amount: &str, token: u32, from: Source) -> Usable {
    Usable { amount: dec(amount), token, from, borrowable: Dec::ZERO }
}

/// The kind of market a coin is: a perp on its dex, a pair (its quote from spot meta), an
/// outcome side.
#[test]
fn a_coin_s_surface_is_its_kind_of_market() {
    let mut store = store();
    assert_eq!(store.surface("BTC"), Some(Surface::Perps { dex: "" }));
    assert_eq!(store.surface("xyz:XYZ100"), Some(Surface::Perps { dex: "xyz" }));
    assert_eq!(store.surface("#14730"), Some(Surface::Outcomes));
    assert_eq!(store.surface("@107"), None, "a pair's quote is spot meta's to say");
    priced(&mut store);
    assert_eq!(store.surface("@107"), Some(Surface::Spot { quote: 0 }));
    assert_eq!(store.surface("PURR/USDC"), Some(Surface::Spot { quote: 0 }));
    assert_eq!(store.surface("@9999"), None, "a pair spot meta does not list");
}

/// A live unified account (2026-10-08): every market draws on its one balance, spot USDC less
/// its hold, whatever its per-dex states say.
#[test]
fn a_unified_account_draws_every_market_on_spot() {
    let mut store = store();
    let _held = unified(&mut store);
    priced(&mut store);
    let user = address(UNIFIED);
    let pooled = usable("736222.0525745799", 0, Source::Unified);
    for coin in ["BTC", "xyz:XYZ100", "@107", "#14730"] {
        assert_eq!(store.usable(user, coin), Some(pooled.clone()), "{coin}");
    }
    assert_eq!(store.usable(user, "cash:TSLA").map(|usable| usable.token), Some(268), "cash posts USDT0");
}

/// A live standard account: each dex draws on its own withdrawable, spot and outcomes on spot
/// USDC less its hold.
#[test]
fn a_standard_account_draws_each_market_on_its_own_balance() {
    let mut store = store();
    let _held = standard(&mut store);
    priced(&mut store);
    let user = address(STANDARD);
    assert_eq!(store.usable(user, "BTC"), Some(usable("463.943191", 0, Source::Perps { dex: String::new() })));
    assert_eq!(store.usable(user, "xyz:XYZ100"), Some(usable("0.0", 0, Source::Perps { dex: "xyz".to_owned() })));
    assert_eq!(store.usable(user, "@107"), Some(usable("246.63995144", 0, Source::Spot)));
    assert_eq!(store.usable(user, "#14730"), Some(usable("246.63995144", 0, Source::Spot)));
    assert_eq!(store.usable(address(UNIFIED), "BTC"), None, "an account not held");
}

/// Portfolio margin: what it would lend against HYPE comes on top, priced at HYPE's pair's mid:
/// 380 free, and 10 HYPE at 40 lent at 0.65.
#[test]
fn portfolio_margin_adds_what_it_would_lend_at_the_store_s_price() {
    let mut store = store();
    let user = address(STANDARD);
    let _held = store.register(Interest::State { user }).unwrap();
    let spot: SpotState = read(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"500","hold":"120","entryNtl":"0.0"},{"coin":"USDT0","token":268,"total":"80","hold":"0","entryNtl":"0.0"},{"coin":"HYPE","token":150,"total":"10","hold":"0","entryNtl":"0.0"}]}"#,
    );
    let _ = store.read(1, Read::Dexes(&dexes()));
    let _ = store.read(1, Read::SpotState { user, state: &spot });
    let _ = store.read(1, Read::Mode { user, mode: &Abstraction::PortfolioMargin });
    priced(&mut store);
    let hype_at_40: Mids = read(r#"{"@107":"40"}"#);
    let _ = store.read(2, Read::Mids { dex: "", mids: &hype_at_40 });
    let btc = store.usable(user, "BTC").unwrap();
    assert_eq!((btc.amount, btc.borrowable, btc.from), (dec("640"), dec("260"), Source::PortfolioMargin));
    assert_eq!(store.usable(user, "cash:TSLA").unwrap().amount, dec("80"), "USDT0 borrows nothing against HYPE");
}

/// A subaccount is read at its own address with its own mode: the unified master's balance is
/// not its subaccount's (the recorded subaccount, empty, never set to a mode).
#[test]
fn a_subaccount_is_its_own_account() {
    let mut store = store();
    let _master = unified(&mut store);
    let sub = address(UNIFIED_SUB);
    let subs: Vec<serde_json::Value> = read(SUBS_UNIFIED);
    let _held = store.register(Interest::State { user: sub }).unwrap();
    let (main, spot): (ClearinghouseState, SpotState) =
        (read(&subs[0]["clearinghouseState"].to_string()), read(&subs[0]["spotState"].to_string()));
    let _ = store.read(1, Read::Mode { user: sub, mode: &Abstraction::Default });
    let _ = store.read(1, Read::Clearinghouse { user: sub, dex: "", state: &main });
    let _ = store.read(1, Read::SpotState { user: sub, state: &spot });
    assert_eq!(store.usable(sub, "BTC"), Some(usable("0.0", 0, Source::Perps { dex: String::new() })));
    assert_eq!(store.usable(address(UNIFIED), "BTC").unwrap().from, Source::Unified);
    assert_eq!(store.followed(), [address(UNIFIED), sub]);
}

/// The live unified account's worth against the venue's own (`portfolio`, read within seconds):
/// its perps are inside its spot, counted once.
#[test]
fn a_unified_account_s_worth_is_the_venue_s() {
    let mut store = store();
    let _held = unified(&mut store);
    priced(&mut store);
    let worth = store.worth(address(UNIFIED), &Beside::default()).unwrap();
    assert_eq!((worth.perps, worth.spot), (dec("549463.356168"), dec("729799.2062150256144")));
    let total = worth.total().unwrap();
    let venue = venue_says(PORTFOLIO_UNIFIED);
    assert!(venue.checked_sub(total).unwrap().abs() < dec("0.001"), "{total} against the venue's {venue}");
}

/// The live standard account's worth with its two vault deposits beside, against the venue's.
#[test]
fn a_standard_account_s_worth_is_the_venue_s() {
    let mut store = store();
    let _held = standard(&mut store);
    priced(&mut store);
    let deposits: Vec<serde_json::Value> = read(crate::live::store::fixtures::VAULTS_STANDARD);
    let vaults = deposits
        .iter()
        .map(|one| VaultShare {
            vault: address(one["vaultAddress"].as_str().unwrap()),
            equity: dec(one["equity"].as_str().unwrap()),
        })
        .collect();
    let worth = store.worth(address(STANDARD), &Beside { vaults, ..Beside::default() }).unwrap();
    let total = worth.total().unwrap();
    assert_eq!(total, dec("367751.0234586470572"));
    let venue = venue_says(PORTFOLIO_STANDARD);
    assert!(venue.checked_sub(total).unwrap().abs() < dec("0.1"), "{total} against the venue's {venue}");
    assert!(store.worth(address(UNIFIED), &Beside::default()).is_none(), "an account not held");
}

/// A deposit one followed account holds in a vault that is itself followed is counted once, as
/// the vault.
#[test]
fn several_accounts_are_counted_once() {
    let mut store = store();
    let (depositor, vault) = (address(STANDARD), address(UNIFIED));
    let _held: Vec<Handle> =
        [depositor, vault].into_iter().map(|user| store.register(Interest::State { user }).unwrap()).collect();
    for (user, total) in [(depositor, "50"), (vault, "1000")] {
        let spot: SpotState = read(&format!(
            r#"{{"balances":[{{"coin":"USDC","token":0,"total":"{total}","hold":"0","entryNtl":"0.0"}}]}}"#
        ));
        let _ = store.read(1, Read::SpotState { user, state: &spot });
        let _ = store.read(1, Read::Mode { user, mode: &Abstraction::Disabled });
    }
    let into = Beside { vaults: vec![VaultShare { vault, equity: dec("100") }], ..Beside::default() };
    let together = store.combined(&[(depositor, into.clone()), (vault, Beside::default())]).unwrap();
    assert_eq!((together.vaults, together.total()), (Dec::ZERO, Some(dec("1050"))));
    let alone = store.combined(&[(depositor, into)]).unwrap();
    assert_eq!(alone.total(), Some(dec("150")), "a vault left out is the depositor's");
}

/// Positions and resting orders dex by dex, the first dex first.
#[test]
fn positions_and_orders_dex_by_dex() {
    let mut store = store();
    let user = address(UNIFIED);
    let _held = unified(&mut store);
    let coins: Vec<&str> = store.positions(user).iter().map(|position| position.coin.as_str()).collect();
    assert_eq!(coins, ["BTC", "ETH", "NEAR"]);
    let second = address(crate::live::store::fixtures::SECOND);
    let _orders =
        [String::new(), "xyz".to_owned()].map(|dex| store.register(Interest::Orders { user: second, dex }).unwrap());
    let xyz = crate::live::store::fixtures::changed("openOrders", |json| {
        json["data"]["dex"] = "xyz".into();
        json["data"]["orders"][0]["coin"] = "xyz:XYZ100".into();
    });
    let _ = store.apply(1, &xyz);
    let _ = store.apply(2, &frame("openOrders"));
    let coins: Vec<&str> = store.orders(second).iter().map(|order| order.coin.as_str()).collect();
    assert_eq!(coins, ["BTC", "xyz:XYZ100"]);
}

/// The prices worth is read at: a token at its USDC pair's mid, USDC at one, an outcome side at
/// its own mid.
#[test]
fn prices_come_from_spot_meta_and_the_mids() {
    let mut store = store();
    priced(&mut store);
    let _ = store.apply(1, &frame("allMids"));
    let prices = store.prices();
    assert_eq!(prices.price("USDC"), Some(Dec::ONE));
    assert_eq!(prices.price("HYPE"), Some(dec("82.897")), "HYPE at @107's mid as the socket last said");
    assert_eq!(prices.price("+14730"), Some(dec("0.612995")), "a side held as + at its # mid");
    assert_eq!(prices.price("MUNCH"), Some(dec("0.00001274")), "MUNCH at @116");
    assert_eq!(prices.price("NOPE"), None, "a token with no pair spot meta lists");
}
