//! An account's clearinghouse frames: one dex's, every dex's at once, its spot balances, and what
//! it may trade on one perp, each in the shape the `/info` read answers.

use crate::account::holdings::{Leverage, MarginMode};
use crate::live::Frame;
use crate::live::fixtures::{FIRST, SECOND, address, dec, mainnet, portfolio, read};
use crate::utils::Dec;

/// One dex's clearinghouse and every dex's read alike. The 2026-09-26 recording holds an isolated
/// position, which carries its raw USD.
#[test]
fn a_dex_s_clearinghouse_and_every_dex_s_read_alike() {
    let Frame::ClearinghouseState(state) = read(&mainnet("clearinghouseState")) else { panic!("a state") };
    assert_eq!((state.dex.as_str(), state.user), ("", address(SECOND)));
    let house = &state.state;
    assert_eq!(
        (house.margin_summary.account_value, house.margin_summary.total_raw_usd),
        (dec("99248.48428"), dec("1071327.7162500001"))
    );
    assert_eq!(
        (house.cross_maintenance_margin_used, house.withdrawable, house.time),
        (dec("12150.990399"), Dec::ZERO, Some(1_791_480_487_027))
    );
    assert_eq!(house.asset_positions.len(), 1);
    let position = &house.asset_positions[0].position;
    assert_eq!(
        (house.asset_positions[0].kind.as_str(), position.coin.as_str(), position.szi),
        ("oneWay", "BTC", dec("-12.07087"))
    );
    assert_eq!(
        (position.leverage.mode(), position.leverage.value(), position.max_leverage),
        (MarginMode::Cross, 11, 40)
    );
    assert_eq!((position.entry_px, position.liquidation_px), (Some(dec("81063.7")), Some(dec("87657.4305040273"))));
    assert_eq!(position.cum_funding.as_ref().map(|paid| paid.all_time), Some(dec("-12707.648999")));

    let Frame::AllDexsClearinghouseState(every) = read(&mainnet("allDexsClearinghouseState")) else { panic!("states") };
    let dexes: Vec<&str> = every.states.iter().map(|(dex, _)| dex.as_str()).collect();
    assert_eq!(dexes, ["", "abcd", "cash", "flx", "hyna", "io", "km", "mkts", "para", "vntl", "xyz"]);
    assert_eq!((every.user, every.states[0].1.asset_positions.len()), (address(SECOND), 1));

    let Frame::AllDexsClearinghouseState(older) = read(&portfolio("allDexsClearinghouseState")) else {
        panic!("states")
    };
    assert_eq!(older.user, address("0x8d1548a936241928cef288a1c377919a5d511f7a"));
    let btc = &older.states[0].1.asset_positions[0].position;
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    assert_eq!(btc.leverage.mode(), MarginMode::Isolated);
    assert_eq!(older.states[0].1.margin_summary.account_value, dec("8456.677271"));
    assert_eq!(older.states[1].0, "xyz");
}

/// What an account may trade on a perp reads both sides; spot balances read with their tokens,
/// and the socket does not say whether portfolio margin is on.
#[test]
fn what_an_account_may_trade_and_its_spot_balances_read_both_sides() {
    let Frame::AssetData(data) = read(&mainnet("activeAssetData")) else { panic!("asset data") };
    assert_eq!((data.user, data.coin.as_str()), (address(FIRST), "BTC"));
    assert_eq!((data.leverage.mode(), data.leverage.value(), data.leverage.raw_usd()), (MarginMode::Cross, 40, None));
    assert_eq!(data.max_trade_szs, [dec("6.95458"), dec("8.53351")]);
    assert_eq!(data.available_to_trade, [dec("13988.094483"), dec("17163.875338")]);
    assert_eq!(data.mark_px, dec("80454.0"));

    let Frame::SpotState(spot) = read(&mainnet("spotState")) else { panic!("spot") };
    assert_eq!((spot.user, spot.state.balances.len()), (address(FIRST), 4));
    let usdc = &spot.state.balances[0];
    assert_eq!(
        (usdc.coin.as_str(), usdc.token, usdc.total, usdc.hold),
        ("USDC", Some(0), dec("19042.782306"), dec("9001.9958"))
    );
    assert_eq!(spot.state.token_to_available_after_maintenance, [(0, dec("16840.184972"))]);
    assert_eq!(spot.state.portfolio_margin_enabled, None, "the socket does not say");
}
