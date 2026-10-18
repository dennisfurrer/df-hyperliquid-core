use crate::account::holdings::{Leverage, MarginMode};
use crate::live::fixtures::{FIRST, SECOND, address, dec, mainnet, portfolio, read};
use crate::live::{Frame, decode};
use crate::utils::Dec;

#[test]
fn what_an_account_may_trade_reads_both_sides() {
    let Frame::AssetData(data) = read(&mainnet("activeAssetData")) else { panic!("asset data") };
    assert_eq!((data.user, data.coin.as_str()), (address(FIRST), "BTC"));
    assert_eq!((data.leverage.mode(), data.leverage.value(), data.leverage.raw_usd()), (MarginMode::Cross, 40, None));
    assert_eq!(data.max_trade_szs, [dec("6.95458"), dec("8.53351")]);
    assert_eq!(data.available_to_trade, [dec("13988.094483"), dec("17163.875338")]);
    assert_eq!(data.mark_px, dec("80454.0"));
}

/// Every recorded `activeAssetData`, read or sent, carries the mark: one without it is refused,
/// not read as a mark unknown.
#[test]
fn what_an_account_may_trade_without_the_mark_is_refused() {
    let sent = mainnet("activeAssetData");
    let without = sent.replace(r#","markPx":"80454.0""#, "");
    assert_ne!(without, sent);
    assert!(decode(&without).is_err());
}

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

    // The 2026-09-26 recording: an isolated position, which carries its raw USD.
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

/// An isolated position always carries its `rawUsd`, as every recorded one does: one without it is
/// refused, never read as none put up.
#[test]
fn an_isolated_position_without_its_raw_usd_is_refused_not_read_as_zero() {
    let sent = portfolio("allDexsClearinghouseState");
    assert!(decode(&sent).is_ok());
    let without = sent.replace(r#","rawUsd":"-3828.81778""#, "");
    assert_ne!(without, sent);
    assert!(decode(&without).is_err());
}

/// Every recorded spot balance, sent or read, carries its entry notional: one without it is
/// refused, not read as a cost unknown.
#[test]
fn a_spot_balance_without_its_entry_notional_is_refused() {
    let sent = mainnet("spotState");
    assert!(decode(&sent).is_ok());
    let without = sent.replacen(r#","entryNtl":"#, r#","entry":"#, 1);
    assert_ne!(without, sent);
    assert!(decode(&without).is_err());
}
