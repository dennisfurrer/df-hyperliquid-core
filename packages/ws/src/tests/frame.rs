use hl_core::{Accepted, Address, Dec, ExchangeResponse, Json, Status};

use super::*;
use crate::{
    account::{Fill, FillLiquidation, Margin, TwapState, UserEvent},
    fixtures::{all, docs, hip4_tui, mainnet, testnet},
    market::{Book, Level, Side},
    subscription::{Interval, Subscription},
};

const FIRST: &str = "0x42f1b36fb856769e075dfbf25938464fb3c92c70";
const SECOND: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn level(px: &str, sz: &str, n: u32) -> Level {
    Level { px: dec(px), sz: dec(sz), n }
}

fn read(text: &str) -> Frame {
    decode(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

#[test]
fn every_recorded_frame_reads_and_none_is_left_unknown() {
    for (name, text) in all() {
        let frame = decode(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!matches!(frame, Frame::Other { .. }), "{name} fell through to Other");
    }
}

#[test]
fn a_book_keeps_both_sides_exactly_and_best_first() {
    let Frame::Book(book) = read(&mainnet("l2Book")) else { panic!("a book") };
    assert_eq!((book.coin.as_str(), book.time, book.spread), ("BTC", 1_791_480_444_439, None));
    assert_eq!((book.bids.len(), book.asks.len()), (20, 20), "twenty levels a side, as the docs say");
    assert_eq!(book.bids[0], level("80459.0", "16.87185", 49));
    assert_eq!(book.bids[3], level("80456.0", "0.0411", 2));
    assert_eq!(book.asks[19], level("80479.0", "6.04591", 3));
    assert!(book.bids.windows(2).all(|pair| pair[0].px > pair[1].px), "bids fall away from the best");
    assert!(book.asks.windows(2).all(|pair| pair[0].px < pair[1].px), "asks rise away from the best");

    let Frame::Book(grouped) = read(&mainnet("l2Book.sigFigs")) else { panic!("a book") };
    assert_eq!(grouped.spread, Some(dec("0.1")), "a grouped book says its spread");
    assert_eq!((grouped.bids[0], grouped.asks[0]), (level("2410.2", "655.1567", 58), level("2410.4", "3.544", 2)));

    let Frame::Book(fast) = read(&mainnet("l2Book.fast")) else { panic!("a book") };
    assert_eq!((fast.bids.len(), fast.asks.len()), (5, 5), "five levels a side when fast");
    assert_eq!(fast.bids[0], level("106.55", "197.7", 2));

    let Frame::Book(empty) = read(&mainnet("l2Book.empty")) else { panic!("a book") };
    assert_eq!((empty.coin.as_str(), empty.bids.len(), empty.asks.len()), ("#14720", 0, 0));
}

#[test]
fn trades_read_each_print_with_its_aggressor_buyer_and_seller() {
    let Frame::Trades(prints) = read(&mainnet("trades")) else { panic!("trades") };
    assert_eq!(prints.len(), 2);
    let first = &prints[0];
    assert_eq!((first.coin.as_str(), first.side, first.px, first.sz), ("BTC", Side::Sell, dec("80600.0"), dec("0.16")));
    assert_eq!((first.time, first.tid), (1_791_480_516_231, 717_978_735_161_105));
    assert_eq!(first.hash, "0x33062daa8fe10fcd347f0446200d470206e300902ae42e9fd6ced8fd4ee4e9b7");
    assert_eq!(first.buyer, address("0x654086857e1fad6dcf05cf6695cce51ea3984268"));
    assert_eq!(first.seller, address("0xa168f51d1ec7f7a4cd38e9b6a461b05f64acfac8"));
    assert_eq!(prints[1].sz, dec("0.84"));

    let Frame::Trades(spot) = read(&mainnet("trades.spot")) else { panic!("trades") };
    assert_eq!((spot[0].coin.as_str(), spot[0].side, spot[0].px), ("@107", Side::Buy, dec("82.894")));
    assert_eq!(read(&mainnet("trades.empty")), Frame::Trades(Vec::new()));
}

#[test]
fn bbo_mids_and_candles_read_as_sent() {
    let Frame::Bbo(bbo) = read(&mainnet("bbo")) else { panic!("a bbo") };
    assert_eq!((bbo.coin.as_str(), bbo.time), ("BTC", 1_791_480_444_656));
    assert_eq!((bbo.bid, bbo.ask), (Some(level("80459.0", "18.36712", 48)), Some(level("80460.0", "1.59648", 9))));
    // The docs' WsBbo: either side may be null.
    let one_sided =
        read(r#"{"channel":"bbo","data":{"coin":"BTC","time":1,"bbo":[null,{"px":"1.5","sz":"2","n":1}]}}"#);
    let Frame::Bbo(one_sided) = one_sided else { panic!("a bbo") };
    assert_eq!((one_sided.bid, one_sided.ask), (None, Some(level("1.5", "2", 1))));

    let Frame::AllMids(mids) = read(&mainnet("allMids")) else { panic!("mids") };
    assert_eq!((mids.dex.as_str(), mids.mids.len()), ("", 1239));
    assert_eq!(mids.mids.iter().next(), Some(("#14720", dec("0.5"))), "kept in the order sent");
    assert_eq!(
        (mids.get("@1"), mids.get("#14730"), mids.get("nothing")),
        (Some(dec("11.9515")), Some(dec("0.612995")), None)
    );
    let Frame::AllMids(xyz) = read(&mainnet("allMids.xyz")) else { panic!("mids") };
    assert_eq!((xyz.dex.as_str(), xyz.mids.len(), xyz.get("xyz:AAPL")), ("xyz", 132, Some(dec("338.095"))));

    let Frame::Candle(candle) = read(&mainnet("candle")) else { panic!("a candle") };
    assert_eq!((candle.open_time, candle.close_time), (1_791_480_420_000, 1_791_480_479_999));
    assert_eq!((candle.coin.as_str(), candle.interval, candle.trades), ("BTC", Interval::OneMinute, 241));
    assert_eq!(
        [candle.open, candle.high, candle.low, candle.close, candle.volume],
        [dec("80429.0"), dec("80470.0"), dec("80416.0"), dec("80459.0"), dec("30.8389")]
    );
}

#[test]
fn a_perp_and_a_spot_context_come_on_their_own_channels() {
    let Frame::AssetCtx { coin, ctx } = read(&mainnet("activeAssetCtx")) else { panic!("a perp context") };
    assert_eq!(coin, "BTC");
    assert_eq!((ctx.funding, ctx.open_interest), (dec("0.0000078567"), dec("39907.5661399998")));
    assert_eq!((ctx.mark_px, ctx.oracle_px, ctx.prev_day_px), (dec("80454.0"), dec("80491.9"), dec("83180.0")));
    assert_eq!((ctx.mid_px, ctx.premium), (Some(dec("80459.5")), Some(dec("-0.0003826472"))));
    assert_eq!(ctx.impact_pxs, Some(vec![dec("80459.0"), dec("80461.1")]));
    assert_eq!((ctx.day_ntl_vlm, ctx.day_base_vlm), (dec("3316947851.6990442276"), Some(dec("40346.9258300001"))));

    let Frame::SpotAssetCtx { coin, ctx } = read(&mainnet("activeSpotAssetCtx")) else { panic!("a spot context") };
    assert_eq!((coin.as_str(), ctx.coin.as_str()), ("@107", "@107"));
    assert_eq!((ctx.mark_px, ctx.mid_px), (dec("82.858"), Some(dec("82.848"))));
    assert_eq!(ctx.circulating_supply, dec("302175679.3606832623"));
    assert_eq!(ctx.total_supply, Some(dec("998874734.525023818")));

    let Frame::SpotAssetCtx { coin, ctx } = read(&mainnet("activeSpotAssetCtx.outcome")) else { panic!("an outcome") };
    assert_eq!((coin.as_str(), ctx.mid_px, ctx.mark_px), ("#14720", None, dec("0.5")));
    assert_eq!(ctx.total_supply, Some(dec("184467440737095.53125")), "eight places past an f64's reach");
}

#[test]
fn the_contexts_of_every_dex_one_dex_and_spot_read_alike() {
    let Frame::AllDexsAssetCtxs(dexes) = read(&mainnet("allDexsAssetCtxs")) else { panic!("contexts") };
    let names: Vec<&str> = dexes.iter().map(|(dex, _)| dex.as_str()).collect();
    assert_eq!(names, ["", "abcd"]);
    assert_eq!(dexes[0].1[1].mark_px, dec("2410.3"));
    let quiet = &dexes[1].1[0];
    assert_eq!((quiet.premium, quiet.mid_px, quiet.impact_pxs.clone()), (None, None, None));

    let Frame::AssetCtxs { dex, ctxs } = read(&mainnet("assetCtxs")) else { panic!("contexts") };
    assert_eq!((dex.as_str(), ctxs.len(), ctxs[0].mark_px), ("xyz", 2, dec("30567.0")));

    let Frame::SpotAssetCtxs(spot) = read(&mainnet("spotAssetCtxs")) else { panic!("contexts") };
    assert_eq!((spot[0].coin.as_str(), spot[1].coin.as_str()), ("PURR/USDC", "@1"));
    assert_eq!(spot[0].circulating_supply, dec("594775181.7359199524"));
}

#[test]
fn what_an_account_may_trade_reads_both_sides() {
    let Frame::AssetData(data) = read(&mainnet("activeAssetData")) else { panic!("asset data") };
    assert_eq!((data.user, data.coin.as_str()), (address(FIRST), "BTC"));
    assert_eq!((data.leverage.margin, data.leverage.value, data.leverage.raw_usd), (Margin::Cross, 40, None));
    assert_eq!(data.max_trade_szs, [dec("6.95458"), dec("8.53351")]);
    assert_eq!(data.available_to_trade, [dec("13988.094483"), dec("17163.875338")]);
    assert_eq!(data.mark_px, Some(dec("80454.0")));
}

fn rebate() -> Fill {
    Fill {
        coin: "BTC".to_owned(),
        px: dec("80499.0"),
        sz: dec("0.00124"),
        side: Side::Sell,
        time: 1_791_480_468_944,
        start_position: dec("-11.86858"),
        dir: "Open Short".to_owned(),
        closed_pnl: Dec::ZERO,
        hash: "0xa36c5669b8ada710a4e60446200ab702061f004f53a0c5e2473501bc77a180fb".to_owned(),
        oid: 569_017_662_681,
        crossed: false,
        fee: dec("-0.002994"),
        tid: 167_376_889_247_413,
        cloid: Some("0x9c7328918712fb838952e96a69ea150d".to_owned()),
        fee_token: "USDC".to_owned(),
        builder_fee: None,
        liquidation: None,
        twap_id: None,
    }
}

#[test]
fn fills_say_whether_they_are_the_snapshot_and_keep_every_field() {
    let Frame::UserFills(snapshot) = read(&mainnet("userFills.snapshot")) else { panic!("fills") };
    assert!(snapshot.is_snapshot, "the first frame after subscribing replays history");
    assert_eq!((snapshot.user, snapshot.fills.len()), (address(FIRST), 30));
    let first = &snapshot.fills[0];
    assert_eq!((first.px, first.sz, first.side, first.crossed), (dec("80395.0"), dec("0.00014"), Side::Buy, true));
    assert_eq!((first.start_position, first.dir.as_str(), first.fee), (dec("0.66492"), "Open Long", dec("0.005064")));
    assert_eq!(first.cloid.as_deref(), Some("0x000000000000000000065d578980bfcb"));

    let Frame::UserFills(stream) = read(&mainnet("userFills")) else { panic!("fills") };
    assert!(!stream.is_snapshot, "a frame without isSnapshot is new fills");
    assert_eq!((stream.user, stream.fills.as_slice()), (address(SECOND), [rebate()].as_slice()));

    let Frame::UserFills(liquidated) = read(&mainnet("userFills.liquidation")) else { panic!("fills") };
    let fill = &liquidated.fills[18];
    assert_eq!(
        fill.liquidation,
        Some(FillLiquidation {
            liquidated_user: Some(address("0xf467f1f46cad3f1041c765765be2891245144f6e")),
            mark_px: dec("80429.0"),
            method: "market".to_owned(),
        })
    );
    assert_eq!((fill.dir.as_str(), fill.closed_pnl), ("Close Short", dec("16.680024")));
    // One market order closing into three liquidations at the same mark.
    let users: Vec<_> = liquidated
        .fills
        .iter()
        .filter_map(|fill| fill.liquidation.as_ref())
        .map(|liquidation| (liquidation.liquidated_user, liquidation.mark_px))
        .collect();
    assert_eq!(
        users,
        [
            (Some(address("0xf467f1f46cad3f1041c765765be2891245144f6e")), dec("80429.0")),
            (Some(address("0xf65ae6061b0ecbddafb3e04381e4fd8c2cd2f0fd")), dec("80429.0")),
            (Some(address("0xfc472f2c3e4227c332ae3b15d796970b4c1afce4")), dec("80429.0")),
        ]
    );
}

#[test]
fn order_updates_and_historical_orders_carry_status_and_order() {
    let Frame::OrderUpdates(updates) = read(&mainnet("orderUpdates")) else { panic!("updates") };
    assert_eq!(updates.len(), 1);
    let update = &updates[0];
    assert_eq!((update.status.as_str(), update.status_timestamp), ("open", 1_791_480_448_890));
    let order = &update.order;
    assert_eq!(
        (order.coin.as_str(), order.side, order.limit_px, order.sz),
        ("BTC", Side::Buy, dec("80462.0"), dec("0.20705"))
    );
    assert_eq!((order.oid, order.timestamp, order.orig_sz), (569_017_209_592, 1_791_480_448_890, dec("0.20705")));
    assert_eq!(order.cloid.as_deref(), Some("0x273d565c96b2523ae6a0b5ca299c6f00"));

    let Frame::HistoricalOrders(history) = read(&mainnet("userHistoricalOrders")) else { panic!("history") };
    assert_eq!((history.user, history.is_snapshot, history.orders.len()), (address(SECOND), false, 1));
    let past = &history.orders[0];
    assert_eq!(
        (past.status.as_str(), past.order.oid, past.order.tif.as_deref()),
        ("open", 569_017_209_592, Some("Alo"))
    );
}

#[test]
fn user_events_come_on_the_user_channel_one_kind_a_frame() {
    assert_eq!(read(&mainnet("user.fills")), Frame::UserEvent(UserEvent::Fills(vec![rebate()])));
}

#[test]
fn recorded_user_events_read_as_their_kind() {
    let Frame::UserEvent(UserEvent::NonUserCancel(cancels)) = read(&mainnet("user.nonUserCancel")) else {
        panic!("cancels")
    };
    assert_eq!(
        cancels.iter().map(|cancel| (cancel.coin.as_str(), cancel.oid)).collect::<Vec<_>>(),
        [("ZEC", 569_031_862_179)]
    );

    let Frame::UserEvent(UserEvent::TwapHistory(history)) = read(&mainnet("user.twapHistory")) else {
        panic!("history")
    };
    assert_eq!(history.len(), 1);
    let started = &history[0];
    assert_eq!(
        (started.time, started.twap_id, started.status.status.as_str()),
        (1_791_481_178, Some(2_297_697), "activated")
    );
    assert_eq!(
        (started.state.coin.as_str(), started.state.side, started.state.sz, started.state.minutes),
        ("BTC", Side::Sell, dec("13.0"), 10)
    );

    let Frame::UserEvent(UserEvent::TwapSliceFills(slices)) = read(&mainnet("user.twapSliceFills")) else {
        panic!("slices")
    };
    assert_eq!(slices.len(), 1);
    assert_eq!((slices[0].twap_id, slices[0].fill.coin.as_str(), slices[0].fill.px), (2_257_646, "ENA", dec("0.1997")));
    assert_eq!(
        (slices[0].fill.sz, slices[0].fill.fee, slices[0].fill.tid),
        (dec("59.0"), dec("0.00458"), 805_092_930_237_458)
    );

    // On the hour: what a short position was paid.
    let Frame::UserEvent(UserEvent::Funding(funding)) = read(&mainnet("user.funding")) else { panic!("funding") };
    assert_eq!(
        (funding.time, funding.coin.as_str(), funding.usdc, funding.szi),
        (1_791_482_400_023, "ZEC", dec("0.082377"), dec("-5.8"))
    );
    assert_eq!((funding.funding_rate, funding.n_samples), (dec("0.0000125"), None));
}

/// No liquidation was recorded; this one follows the docs' `WsLiquidation` field for field, keys
/// in snake case as the docs write them. One frame holds one event, and a kind this build does not
/// know is kept as written.
#[test]
fn a_liquidation_reads_as_the_docs_type_it_and_an_unknown_event_is_kept() {
    let liquidation = read(
        r#"{"channel":"user","data":{"liquidation":{"lid":7,"liquidator":"0x42f1b36fb856769e075dfbf25938464fb3c92c70","liquidated_user":"0x78dce96236804d6958bf3c4598cf5efbc371629c","liquidated_ntl_pos":"1000.5","liquidated_account_value":"20.25"}}}"#,
    );
    let Frame::UserEvent(UserEvent::Liquidation(liquidation)) = liquidation else { panic!("a liquidation") };
    assert_eq!(
        (liquidation.lid, liquidation.liquidator, liquidation.liquidated_user),
        (7, address(FIRST), address(SECOND))
    );
    assert_eq!((liquidation.liquidated_ntl_pos, liquidation.liquidated_account_value), (dec("1000.5"), dec("20.25")));

    let unknown = read(r#"{"channel":"user","data":{"somethingNew":{"a":1.5}}}"#);
    assert_eq!(
        unknown,
        Frame::UserEvent(UserEvent::Other { key: "somethingNew".to_owned(), data: r#"{"a":1.5}"#.to_owned() })
    );
}

#[test]
fn fundings_and_ledger_updates_keep_their_amounts_exact() {
    let Frame::UserFundings(fundings) = read(&mainnet("userFundings")) else { panic!("fundings") };
    assert_eq!((fundings.user, fundings.is_snapshot, fundings.fundings.len()), (address(FIRST), true, 100));
    let first = &fundings.fundings[0];
    assert_eq!(
        (first.time, first.coin.as_str(), first.usdc, first.szi),
        (1_791_421_200_128, "TAO", dec("-0.025182"), dec("6.917"))
    );
    assert_eq!((first.funding_rate, first.n_samples), (dec("0.0000125"), None));
    let last = &fundings.fundings[99];
    assert_eq!(
        (last.coin.as_str(), last.usdc, last.funding_rate),
        ("xyz:BRENTOIL", dec("-8.101769"), dec("-0.0003825775"))
    );

    let Frame::Ledger(ledger) = read(&mainnet("userNonFundingLedgerUpdates")) else { panic!("ledger") };
    assert_eq!((ledger.user, ledger.is_snapshot, ledger.updates.len()), (address(FIRST), true, 23));
    let deposit = &ledger.updates[0];
    assert_eq!(
        (deposit.time, deposit.kind(), deposit.dec("usdc")),
        (1_788_831_761_730, Some("deposit"), Some(dec("7.0")))
    );
    assert_eq!(deposit.hash, "0x885308289db123a4efc1afb1aa1070955311838659bd00672be3005d450b7fb4");
    let send = &ledger.updates[1];
    assert_eq!(
        (send.kind(), send.dec("amount"), send.dec("fee")),
        (Some("send"), Some(dec("9.75306")), Some(Dec::ZERO))
    );
    assert_eq!(send.delta.get("destinationDex").and_then(Json::as_str), Some(""));
    assert_eq!(send.delta.get("nonce").and_then(Json::as_u64), Some(3_217_031));
    assert_eq!(send.dec("token"), None, "a name is not an amount");
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
        (dec("12150.990399"), Dec::ZERO, 1_791_480_487_027)
    );
    assert_eq!(house.asset_positions.len(), 1);
    let position = &house.asset_positions[0].position;
    assert_eq!(
        (house.asset_positions[0].kind.as_str(), position.coin.as_str(), position.szi),
        ("oneWay", "BTC", dec("-12.07087"))
    );
    assert_eq!((position.leverage.margin, position.leverage.value, position.max_leverage), (Margin::Cross, 11, 40));
    assert_eq!((position.entry_px, position.liquidation_px), (Some(dec("81063.7")), Some(dec("87657.4305040273"))));
    assert_eq!(position.cum_funding.all_time, dec("-12707.648999"));

    let Frame::AllDexsClearinghouseState(every) = read(&mainnet("allDexsClearinghouseState")) else { panic!("states") };
    let dexes: Vec<&str> = every.states.iter().map(|(dex, _)| dex.as_str()).collect();
    assert_eq!(dexes, ["", "abcd", "cash", "flx", "hyna", "io", "km", "mkts", "para", "vntl", "xyz"]);
    assert_eq!((every.user, every.states[0].1.asset_positions.len()), (address(SECOND), 1));

    // hip4-tui's recording: an isolated position, which carries its raw USD.
    let Frame::AllDexsClearinghouseState(older) = read(&hip4_tui("allDexsClearinghouseState")) else {
        panic!("states")
    };
    assert_eq!(older.user, address("0x8d1548a936241928cef288a1c377919a5d511f7a"));
    let btc = &older.states[0].1.asset_positions[0].position;
    assert_eq!(
        (btc.leverage.margin, btc.leverage.value, btc.leverage.raw_usd),
        (Margin::Isolated, 4, Some(dec("-3828.81778")))
    );
    assert_eq!(older.states[0].1.margin_summary.account_value, dec("8456.677271"));
    assert_eq!(older.states[1].0, "xyz");
}

#[test]
fn open_orders_spot_balances_and_web_data_read_what_they_say() {
    let Frame::OpenOrders(open) = read(&mainnet("openOrders")) else { panic!("orders") };
    assert_eq!((open.dex.as_str(), open.user, open.orders.len()), ("", address(SECOND), 1));
    let order = &open.orders[0];
    assert_eq!(
        (order.limit_px, order.sz, order.orig_sz, order.trigger_px),
        (dec("80459.0"), dec("0.20706"), dec("0.20706"), Dec::ZERO)
    );
    assert_eq!(
        (order.order_type.as_str(), order.tif.as_deref(), order.trigger_condition.as_str()),
        ("Limit", Some("Alo"), "N/A")
    );
    assert!(!order.is_trigger && !order.is_position_tpsl && !order.reduce_only && order.children.is_empty());
    let Frame::OpenOrders(none) = read(&mainnet("openOrders.empty")) else { panic!("orders") };
    assert!(none.orders.is_empty());

    let Frame::SpotState(spot) = read(&mainnet("spotState")) else { panic!("spot") };
    assert_eq!((spot.user, spot.balances.len()), (address(FIRST), 4));
    let usdc = &spot.balances[0];
    assert_eq!(
        (usdc.coin.as_str(), usdc.token, usdc.total, usdc.hold),
        ("USDC", Some(0), dec("19042.782306"), dec("9001.9958"))
    );
    assert_eq!(spot.available_after_maintenance, [(0, dec("16840.184972"))]);

    let Frame::WebData3(web) = read(&mainnet("webData3")) else { panic!("web data") };
    let state = &web.user_state;
    assert_eq!((state.user, state.agent_address, state.agent_valid_until), (address(SECOND), None, None));
    assert_eq!((state.cum_ledger, state.server_time, state.is_vault), (dec("-248771.0"), 1_791_480_444_991, false));
    assert_eq!((state.abstraction.as_deref(), state.dex_abstraction_enabled), (Some("disabled"), false));
    assert_eq!(web.perp_dex_states.len(), 11);
    assert_eq!(
        web.perp_dex_states[0].perps_at_open_interest_cap,
        ["CANTO", "FTM", "JELLY", "LOOM", "RLB", "VINE", "ZEREBRO"]
    );
    assert!(web.perp_dex_states[1].perps_at_open_interest_cap.is_empty());

    assert_eq!(
        read(&mainnet("notification")),
        Frame::Notification("Resting order filled: Bought 0.00124 BTC at $80462".to_owned())
    );
}

#[test]
fn twap_states_slices_and_history_read_as_recorded() {
    let state = TwapState {
        coin: "@107".to_owned(),
        user: address("0x51a4cee5e31cc8f3bef1564b7a2d0005e2775a98"),
        side: Side::Buy,
        sz: dec("5000.0"),
        executed_sz: dec("20.8"),
        executed_ntl: dec("1738.48367"),
        minutes: 10_080,
        reduce_only: false,
        randomize: true,
        timestamp: 1_791_478_452_792,
    };
    let Frame::TwapStates(states) = read(&mainnet("twapStates")) else { panic!("twaps") };
    assert_eq!(
        (states.dex.as_str(), states.user, states.states.as_slice()),
        ("", state.user, [(2_297_432, state)].as_slice())
    );
    let Frame::TwapStates(none) = read(&mainnet("twapStates.empty")) else { panic!("twaps") };
    assert!(none.states.is_empty());

    let Frame::TwapSliceFills(slices) = read(&mainnet("userTwapSliceFills")) else { panic!("slices") };
    assert_eq!(
        (slices.user, slices.is_snapshot, slices.fills.len()),
        (address("0xbe3f79ae0ab3294aaa3230c1155e912c05b6a55b"), false, 1)
    );
    let slice = &slices.fills[0];
    assert_eq!((slice.twap_id, slice.fill.px, slice.fill.twap_id), (2_296_264, dec("80832.0"), None));
    assert_eq!(slice.fill.hash, format!("0x{}", "0".repeat(64)), "a TWAP slice has a zero hash");

    let Frame::TwapHistory(history) = read(&mainnet("userTwapHistory")) else { panic!("history") };
    assert_eq!((history.is_snapshot, history.history.len()), (true, 3));
    let started = &history.history[0];
    assert_eq!(
        (started.time, started.status.status.as_str(), started.status.description.as_deref()),
        (1_791_401_487, "activated", None)
    );
    assert_eq!(
        (started.twap_id, started.state.sz, started.state.coin.as_str()),
        (Some(2_294_081), dec("2831.96"), "HYPE")
    );
    let stopped = &history.history[1];
    assert_eq!(
        (stopped.status.status.as_str(), stopped.state.executed_sz, stopped.state.executed_ntl),
        ("terminated", dec("0.14"), dec("12.355"))
    );
    let Frame::TwapSliceFills(none) = read(&mainnet("userTwapSliceFills.empty")) else { panic!("slices") };
    assert!(none.is_snapshot && none.fills.is_empty());
    let Frame::TwapHistory(none) = read(&mainnet("userTwapHistory.empty")) else { panic!("history") };
    assert!(none.is_snapshot && none.history.is_empty());
}

#[test]
fn outcome_changes_and_fast_contexts_are_kept_as_sent() {
    let Frame::OutcomeMetaUpdates(changes) = read(&testnet("outcomeMetaUpdates")) else { panic!("changes") };
    assert_eq!(changes.compact(), r#"[{"outcomeSettled":23205}]"#);
    let Frame::FastAssetCtxs(payload) = read(&mainnet("fastAssetCtxs")) else { panic!("fast contexts") };
    assert!(payload.starts_with("bVjLruM4Dv2X9LYg6C35"));
    assert!(payload.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')), "base64");
}

#[test]
fn acks_pongs_and_refusals_are_frames_of_their_own() {
    let Frame::Ack(ack) = read(&mainnet("ack.unsubscribe")) else { panic!("an ack") };
    assert_eq!(ack.method, Method::Unsubscribe);
    assert_eq!(ack.echo.compact(), r#"{"type":"trades","coin":"@107"}"#);
    let Frame::Ack(ack) = read(&hip4_tui("ack.allDexsClearinghouseState")) else { panic!("an ack") };
    assert_eq!(ack.method, Method::Subscribe);
    assert_eq!(
        ack.subscription(),
        Ok(Subscription::AllDexsClearinghouseState { user: address("0x8d1548a936241928cef288a1c377919a5d511f7a") })
    );
    assert_eq!(read(&mainnet("pong")), Frame::Pong);

    let refusal = |name: &str| match read(&mainnet(name)) {
        Frame::Error(refusal) => refusal,
        other => panic!("{name} is {other:?}"),
    };
    let subscribed = refusal("error.alreadySubscribed");
    assert_eq!(subscribed.message, r#"Already subscribed: {"type":"trades","coin":"BTC"}"#);
    assert_eq!(
        (subscribed.post_id(), subscribed.subscription()),
        (None, Some(Subscription::Trades { coin: "BTC".to_owned() }))
    );
    assert_eq!(
        refusal("error.alreadyUnsubscribed").subscription(),
        Some(Subscription::Trades { coin: "SOL".to_owned() })
    );
    assert_eq!(
        refusal("error.unsubscribe.fast").subscription(),
        Some(Subscription::L2Book { coin: "SOL".to_owned(), n_sig_figs: None, mantissa: None, fast: false })
    );
    assert_eq!(refusal("error.interval").subscription(), None, "7m is no interval");
    let post = refusal("error.post");
    assert_eq!((post.post_id(), post.subscription()), (Some(2), None));
    for name in ["error.notJson", "error.webData2", "error.invalidSubscription"] {
        assert_eq!((refusal(name).post_id(), refusal(name).subscription()), (None, None), "{name}");
    }
    // @nktkas/hyperliquid 0.33.2 reads a post's id off the end of a refusal such as this one.
    let Frame::Error(pending) = read(r#"{"channel":"error","data":"too many pending post requests id=1234"}"#) else {
        panic!("a refusal")
    };
    assert_eq!(pending.post_id(), Some(1234));
}

#[test]
fn post_answers_are_matched_by_id_and_keep_what_was_answered() {
    let Frame::Post(book) = read(&mainnet("post.info.l2Book")) else { panic!("an answer") };
    let Answer::Info { kind, data } = book.answer else { panic!("an info answer") };
    assert_eq!((book.id, kind.as_str()), (1, "l2Book"));
    let book: Book = serde_json::from_str(&data).unwrap();
    assert_eq!(
        (book.coin.as_str(), book.bids[0], book.spread),
        ("ETH", level("2410.2", "655.1567", 58), Some(dec("0.1")))
    );

    let Frame::Post(state) = read(&mainnet("post.info.clearinghouseState")) else { panic!("an answer") };
    let Answer::Info { kind, data } = state.answer else { panic!("an info answer") };
    assert_eq!((state.id, kind.as_str()), (12, "clearinghouseState"));
    let house: crate::account::Clearinghouse = serde_json::from_str(&data).unwrap();
    assert_eq!(house.margin_summary.account_value, dec("96828.21505"));

    assert_eq!(
        read(&mainnet("post.info.null")),
        Frame::Post(Reply { id: 11, answer: Answer::Info { kind: "l2Book".to_owned(), data: "null".to_owned() } }),
        "an unknown coin's book is null"
    );
    assert_eq!(
        read(&testnet("post.action.err")),
        Frame::Post(Reply {
            id: 22,
            answer: Answer::Action(ExchangeResponse::Err(
                "User or API Wallet 0xf9414817e74c92ea33b3e8614efcc3ed198df423 does not exist.".to_owned()
            )),
        })
    );
    let resting = serde_json::json!({"resting": {"oid": 88383}});
    assert_eq!(
        read(&docs("post.action.ok")),
        Frame::Post(Reply {
            id: 256,
            answer: Answer::Action(ExchangeResponse::Ok(Accepted {
                kind: "order".to_owned(),
                statuses: vec![Status::Ok(resting.clone())],
                data: Some(serde_json::json!({"statuses": [resting]})),
            })),
        })
    );
    // The docs: an error is the text HTTP would have answered.
    assert_eq!(
        read(r#"{"channel":"post","data":{"id":5,"response":{"type":"error","payload":"429 Too Many Requests"}}}"#),
        Frame::Post(Reply { id: 5, answer: Answer::Error("429 Too Many Requests".to_owned()) })
    );
}

#[test]
fn an_unknown_channel_is_kept_as_written_and_is_not_an_error() {
    assert_eq!(
        read(r#"{"channel":"somethingNew","data":{"x":[1,2.50,"3"]}}"#),
        Frame::Other { channel: "somethingNew".to_owned(), data: r#"{"x":[1,2.50,"3"]}"#.to_owned() }
    );
    assert_eq!(read(r#"{"channel":"quiet"}"#), Frame::Other { channel: "quiet".to_owned(), data: String::new() });
}

#[test]
fn what_does_not_read_is_an_error_naming_the_channel() {
    for text in ["", "not json", "Websocket connection established.", "[]", r#"{"data":{}}"#, r#"{"channel":7}"#] {
        let error = decode(text).unwrap_err();
        assert_eq!(error.channel, None, "{text:?}");
    }
    for (text, channel) in [
        (r#"{"channel":"l2Book","data":{"coin":"BTC"}}"#, "l2Book"),
        (r#"{"channel":"l2Book"}"#, "l2Book"),
        (
            r#"{"channel":"trades","data":[{"coin":"BTC","side":"X","px":"1","sz":"1","time":1,"hash":"0x","tid":1,"users":["0x42f1b36fb856769e075dfbf25938464fb3c92c70","0x78dce96236804d6958bf3c4598cf5efbc371629c"]}]}"#,
            "trades",
        ),
        // A price written as a float has been through one; it is refused, never rounded.
        (r#"{"channel":"bbo","data":{"coin":"BTC","time":1,"bbo":[{"px":80459.5,"sz":"1","n":1},null]}}"#, "bbo"),
        (
            r#"{"channel":"candle","data":{"t":1,"T":2,"s":"BTC","i":"7m","o":"1","c":"1","h":"1","l":"1","v":"1","n":1}}"#,
            "candle",
        ),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"other","payload":{}}}}"#, "post"),
        (
            r#"{"channel":"subscriptionResponse","data":{"method":"resubscribe","subscription":{}}}"#,
            "subscriptionResponse",
        ),
    ] {
        let error = decode(text).unwrap_err();
        assert_eq!(error.channel.as_deref(), Some(channel), "{text}");
    }
}

#[test]
fn no_cut_of_a_recorded_frame_panics() {
    for (name, text) in all() {
        let step = if text.len() > 2_000 { 97 } else { 1 };
        for end in (0..text.len()).step_by(step).filter(|end| text.is_char_boundary(*end)) {
            let _ = decode(&text[..end]);
        }
        let _ = decode(&text.replace('"', "'"));
        assert!(decode(&text).is_ok(), "{name}");
    }
}
