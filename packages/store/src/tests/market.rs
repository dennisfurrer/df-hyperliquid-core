use hl_core::{Dec, Network};
use hl_trade::{L2Book, MarketTrade, Mids};
use hl_ws::{Candle, Frame, Interval, Level, Trade};

use crate::{
    BARS, Context, Folded, Handle, Interest, Read, Store,
    fixtures::{
        ALL_MIDS, CANDLE_SNAPSHOT, L2_BOOK, PARA, PERP_META, RECENT_TRADES, SPOT_META_TESTNET, changed, dec, dexes,
        frame, read, sent,
    },
    market::fold_bar,
};

fn store() -> Store {
    Store::new(Network::Mainnet)
}

fn wanting(store: &mut Store, interests: impl IntoIterator<Item = Interest>) -> Vec<Handle> {
    interests.into_iter().map(|interest| store.register(interest).unwrap()).collect()
}

fn coin(name: &str) -> String {
    name.to_owned()
}

fn level(px: &str, sz: &str, n: u32) -> Level {
    Level { px: dec(px), sz: dec(sz), n }
}

/// The recorded BTC book (mainnet, 2026-10-08): its touch, and halfway between, exactly.
#[test]
fn a_book_gives_the_touch_and_the_mid_between() {
    let mut store = store();
    let _book = wanting(&mut store, [Interest::Book { coin: coin("BTC") }]);
    assert_eq!(store.apply(1_000, &frame("l2Book")), Folded::Changed);
    assert_eq!(store.best("BTC"), (Some(level("80459.0", "16.87185", 49)), Some(level("80460.0", "4.21967", 21))));
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")));
    let held = store.book("BTC").unwrap();
    assert_eq!((held.time, held.bids.len(), held.heard, held.changed), (1791480444439, 20, 1_000, 1_000));
}

/// A book told again unchanged is heard, not changed: staleness reads `heard`.
#[test]
fn a_book_told_again_is_heard_and_not_changed() {
    let mut store = store();
    let _book = wanting(&mut store, [Interest::Book { coin: coin("BTC") }]);
    assert_eq!(store.apply(1_000, &frame("l2Book")), Folded::Changed);
    assert_eq!(store.apply(4_000, &frame("l2Book")), Folded::Same);
    let held = store.book("BTC").unwrap();
    assert_eq!((held.heard, held.changed, held.age(10_000)), (4_000, 1_000, 6_000));
}

/// `allMids` covers every coin: the first dex's carries spot pairs and outcome sides too, and a
/// HIP-3 dex's its own. Mids are kept whether or not a page asked for them.
#[test]
fn the_venue_s_mids_cover_every_coin() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("allMids")), Folded::Changed);
    assert_eq!(store.apply(1_000, &frame("allMids.xyz")), Folded::Changed);
    assert_eq!(store.mid("BTC"), Some(dec("80474.5")));
    assert_eq!(store.mid("@107"), Some(dec("82.897")));
    assert_eq!(store.mid("#14720"), Some(dec("0.5")));
    assert_eq!(store.mid("xyz:AAPL"), Some(dec("338.095")));
    assert_eq!(store.mid("xyz:NOPE"), None);
    assert_eq!(store.mids("xyz").unwrap().heard, 1_000);
    let mids: Mids = read(ALL_MIDS);
    assert_eq!(store.read(2_000, Read::Mids { dex: "", mids: &mids }), Folded::Changed);
    assert_eq!(store.mid("BTC"), Some(dec("80847.5")), "a read replaces the dex's mids");
    assert_eq!(store.mid("SOL"), None, "the read named no SOL");
}

/// A book held is the market a page watches, sent on every change: its mid wins over the
/// venue's list. An empty book (an outcome side no one quotes) falls back to the list.
#[test]
fn a_book_s_mid_wins_and_an_empty_book_falls_back_to_the_venue_s() {
    let mut store = store();
    let _books = wanting(&mut store, [Interest::Book { coin: coin("BTC") }, Interest::Book { coin: coin("#14720") }]);
    let _ = store.apply(1_000, &frame("allMids"));
    let _ = store.apply(1_000, &frame("l2Book"));
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")));
    assert_eq!(store.apply(1_000, &frame("l2Book.empty")), Folded::Changed);
    assert_eq!(store.best("#14720"), (None, None));
    assert_eq!(store.mid("#14720"), Some(dec("0.5")));
}

/// A book no page wants is not kept.
#[test]
fn a_book_no_one_wants_is_dropped() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("l2Book")), Folded::Dropped);
    assert!(store.book("BTC").is_none());
    let book: L2Book = read(L2_BOOK);
    assert_eq!(store.read(1_000, Read::Book(&book)), Folded::Dropped);
}

/// A book carries no sequence; its own time orders it. The REST read (06:53:45) is newer than
/// the socket's frame (06:47:24): the frame arriving after it is older news and is dropped.
#[test]
fn an_older_book_never_replaces_a_newer_one() {
    let mut store = store();
    let _book = wanting(&mut store, [Interest::Book { coin: coin("BTC") }]);
    let book: L2Book = read(L2_BOOK);
    assert_eq!(store.read(1_000, Read::Book(&book)), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("l2Book")), Folded::Dropped);
    assert_eq!(store.book("BTC").unwrap().time, 1791480825902);
    assert_eq!(store.best("BTC").0, Some(level("80858.0", "23.7926", 61)));
}

/// A grouped book (`nSigFigs`, sent with a spread) is not the full book the store asks for, so
/// it never stands in for one.
#[test]
fn a_grouped_book_is_not_the_full_book() {
    let mut store = store();
    let _book = wanting(&mut store, [Interest::Book { coin: coin("ETH") }]);
    assert_eq!(store.apply(1_000, &frame("l2Book.sigFigs")), Folded::Dropped);
    assert!(store.book("ETH").is_none());
}

/// Releasing the last interest in a book lets the book go.
#[test]
fn a_book_goes_with_its_last_interest() {
    let mut store = store();
    let mut held = wanting(&mut store, [Interest::Book { coin: coin("BTC") }]);
    let _ = store.apply(1_000, &frame("l2Book"));
    store.release(held.remove(0));
    assert!(store.book("BTC").is_none());
    assert_eq!(store.best("BTC"), (None, None));
}

/// Prints are kept newest first. The socket sends them oldest first; `recentTrades` answers
/// newest first; each print is held once, by its coin, `tid` and time.
#[test]
fn prints_are_kept_newest_first_and_once() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
    let recent: Vec<MarketTrade> = read(RECENT_TRADES);
    assert_eq!(store.read(1_000, Read::Trades(&recent)), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("trades")), Folded::Changed);
    assert_eq!(store.apply(3_000, &frame("trades")), Folded::Same, "the same prints again");
    assert_eq!(store.read(3_000, Read::Trades(&recent)), Folded::Same);
    let tape = store.trades("BTC").unwrap();
    let held: Vec<(u64, u64)> = tape.iter().map(|print| (print.time, print.tid)).collect();
    assert_eq!(
        held,
        [
            (1791480719939, 739564670599029),
            (1791480719435, 958487846715329),
            (1791480719236, 966065094835399),
            (1791480516231, 8986569314831),
            (1791480516231, 717978735161105),
        ]
    );
    assert_eq!((tape.heard, tape.changed), (3_000, 2_000));
    assert_eq!(store.apply(3_000, &frame("trades.spot")), Folded::Dropped, "no page wants @107's");
}

/// The recorded BTC print with its time and `tid` set: for hip4-tui's prints, which name only
/// a time.
fn print(time: u64) -> Trade {
    let Frame::Trades(prints) = frame("trades") else { panic!("prints") };
    Trade { time, tid: time, ..prints[0].clone() }
}

/// hip4-tui: prints come oldest first and are kept newest first.
#[test]
fn prints_come_oldest_first_and_are_kept_newest_first() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
    let _ = store.apply(1, &Frame::Trades(vec![print(1), print(2)]));
    let _ = store.apply(2, &Frame::Trades(vec![print(3)]));
    let times: Vec<u64> = store.trades("BTC").unwrap().iter().map(|print| print.time).collect();
    assert_eq!(times, [3, 2, 1]);
}

#[test]
fn the_tape_stops_at_its_count() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
    let prints: Vec<Trade> = (1..=250).map(print).collect();
    let _ = store.apply(1, &Frame::Trades(prints));
    let tape = store.trades("BTC").unwrap();
    assert_eq!(tape.len(), crate::TAPE);
    assert_eq!((tape[0].time, tape[crate::TAPE - 1].time), (250, 51), "the oldest go");
}

/// The recorded BTC minute candle, opening at `start` and closing at `close`.
fn bar(start: u64, close: &str) -> Candle {
    let Frame::Candle(candle) = frame("candle") else { panic!("a candle") };
    Candle { open_time: start, close: dec(close), ..candle }
}

/// hip4-tui: a candle replaces the one forming and the next goes on the end.
#[test]
fn a_candle_replaces_the_one_forming_and_the_next_goes_on_the_end() {
    let mut bars = vec![bar(0, "1"), bar(900, "2")];
    assert!(fold_bar(&mut bars, bar(900, "3")));
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[1].close, dec("3"), "the one forming is replaced");
    assert!(fold_bar(&mut bars, bar(1800, "4")));
    assert_eq!(bars.len(), 3, "the next is added");
    assert!(fold_bar(&mut bars, bar(0, "5")));
    assert_eq!(bars[0].close, dec("5"), "a late one lands on its own start");
    assert_eq!(bars.len(), 3);
    assert!(!fold_bar(&mut bars, bar(0, "5")), "the same again changes nothing");
}

/// hip4-tui: the candles stop at their count, the oldest going.
#[test]
fn the_candles_stop_at_their_count() {
    let mut bars = Vec::new();
    for at in 0..(BARS as u64 + 10) {
        let _ = fold_bar(&mut bars, bar(at * 900, "1"));
    }
    assert_eq!(bars.len(), BARS);
    assert_eq!(bars[0].open_time, 10 * 900, "the oldest go");
}

/// The recorded 1m candle, and the recorded hour candles read, each kept at its own interval.
#[test]
fn candles_are_kept_a_market_and_interval_each() {
    let mut store = store();
    let _candles = wanting(
        &mut store,
        [
            Interest::Candles { coin: coin("BTC"), interval: Interval::OneMinute },
            Interest::Candles { coin: coin("BTC"), interval: Interval::OneHour },
        ],
    );
    assert_eq!(store.apply(1_000, &frame("candle")), Folded::Changed);
    let history: Vec<hl_trade::Candle> = read(CANDLE_SNAPSHOT);
    assert_eq!(store.read(1_000, Read::Candles(&history)), Folded::Changed);
    let minute = store.candles("BTC", Interval::OneMinute).unwrap();
    assert_eq!((minute.len(), minute[0].open_time, minute[0].close), (1, 1791480420000, dec("80459.0")));
    let hours: Vec<(u64, Dec)> =
        store.candles("BTC", Interval::OneHour).unwrap().iter().map(|one| (one.open_time, one.close)).collect();
    assert_eq!(
        hours,
        [
            (1791468000000, dec("82678.0")),
            (1791471600000, dec("81009.0")),
            (1791475200000, dec("80929.0")),
            (1791478800000, dec("80867.0")),
        ]
    );
    assert!(store.candles("BTC", Interval::FiveMinutes).is_none());
}

/// hip4-tui: the history read when a market comes on screen goes under what the socket already
/// sent; the live candle wins.
#[test]
fn history_goes_under_what_the_socket_already_sent() {
    let mut store = store();
    let _candles = wanting(&mut store, [Interest::Candles { coin: coin("BTC"), interval: Interval::OneHour }]);
    let history: Vec<hl_trade::Candle> = read(CANDLE_SNAPSHOT);
    let live = Candle {
        interval: Interval::OneHour,
        open_time: 1791478800000,
        close_time: 1791482399999,
        close: dec("80999.0"),
        ..bar(0, "0")
    };
    let _ = store.apply(1_000, &Frame::Candle(live));
    let _ = store.read(2_000, Read::Candles(&history));
    let closes: Vec<Dec> = store.candles("BTC", Interval::OneHour).unwrap().iter().map(|one| one.close).collect();
    assert_eq!(closes, [dec("82678.0"), dec("81009.0"), dec("80929.0"), dec("80999.0")]);
}

/// A perp's context from `activeAssetCtx`, a spot pair's and an outcome side's from
/// `activeSpotAssetCtx`, as recorded.
#[test]
fn a_market_s_context_is_its_kind_s() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("activeAssetCtx")), Folded::Changed);
    assert_eq!(store.apply(1_000, &frame("activeSpotAssetCtx")), Folded::Changed);
    assert_eq!(store.apply(1_000, &frame("activeSpotAssetCtx.outcome")), Folded::Changed);
    let Some(Context::Perp(btc)) = store.context("BTC").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!((btc.mark_px, btc.funding, btc.oracle_px), (dec("80454.0"), dec("0.0000078567"), dec("80491.9")));
    let Some(Context::Spot(hype)) = store.context("@107").map(|ctx| ctx.value) else { panic!("a pair") };
    assert_eq!((hype.mark_px, hype.circulating_supply), (dec("82.858"), Some(dec("302175679.3606832623"))));
    let side = store.context("#14720").unwrap();
    assert_eq!((side.mark(), side.mid(), side.heard), (dec("0.5"), None, 1_000));
    assert!(store.context("ETH").is_none());
    assert_eq!(store.mid("#14720"), None, "no book, no list, and a context with no mid");
}

/// `allDexsAssetCtxs` sends each dex's contexts in its universe's order, which `perpDexs` and
/// `allPerpMetas` give. Recorded the same moment, the context it places on BTC is the one
/// `activeAssetCtx` sent for BTC.
#[test]
fn every_dex_s_contexts_land_on_their_coins() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("allDexsAssetCtxs")), Folded::Dropped, "no universe yet to place them");
    let dexes = dexes();
    assert_eq!(store.read(1_000, Read::Dexes(&dexes)), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("allDexsAssetCtxs")), Folded::Changed);
    let Some(Context::Perp(placed)) = store.context("BTC").map(|ctx| ctx.value) else { panic!("a perp") };
    let placed = placed.clone();
    assert_eq!(store.apply(3_000, &frame("activeAssetCtx")), Folded::Same);
    assert_eq!(store.context("BTC").unwrap().value, Context::Perp(&placed));
    assert_eq!(store.context("BTC").unwrap().changed, 2_000, "the same context, told again");
    let Some(Context::Perp(eth)) = store.context("ETH").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!(eth.mark_px, dec("2410.3"));
    let Some(Context::Perp(abcd)) = store.context("abcd:USA500").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!((abcd.mark_px, abcd.mid_px), (dec("6980.0"), None));
}

/// One dex's contexts (`assetCtxs`), and one dex's read with its meta.
#[test]
fn one_dex_s_contexts_land_on_its_coins() {
    let mut store = store();
    let _ = store.read(1_000, Read::Dexes(&dexes()));
    assert_eq!(store.apply(1_000, &frame("assetCtxs")), Folded::Changed);
    assert_eq!(store.context("xyz:XYZ100").unwrap().mark(), dec("30567.0"));
    assert_eq!(store.context("xyz:TSLA").unwrap().mark(), dec("368.56"));
    let para: hl_hip3::DexMetaAndAssetCtxs = read(PARA);
    assert_eq!(store.read(2_000, Read::Dex { dex: "para", read: &para }), Folded::Changed);
    let Some(Context::Perp(total)) = store.context("para:TOTAL2").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!((total.mark_px, total.mid_px), (dec("1179.574"), Some(dec("1179.65"))));
}

/// The first dex's `metaAndAssetCtxs`, put together from the recorded meta and the recorded
/// contexts of its first two perps.
#[test]
fn the_first_dex_s_read_places_its_contexts() {
    let mut store = store();
    let recorded: serde_json::Value = read(&sent("allDexsAssetCtxs"));
    let first = &recorded["data"]["ctxs"][0][1];
    let meta: serde_json::Value = read(PERP_META);
    let both: hl_perps::MetaAndAssetCtxs = read(&serde_json::json!([meta, first]).to_string());
    assert_eq!(both.ctxs.len(), 2);
    assert_eq!(store.read(1_000, Read::Perps(&both)), Folded::Changed);
    assert_eq!(store.context("ETH").unwrap().mark(), dec("2410.3"));
    assert!(store.context("ATOM").is_none(), "a perp with no context read has none");
    assert_eq!(store.collateral().of(""), 0);
}

/// Every spot pair's and outcome side's context comes keyed by its coin.
#[test]
fn spot_contexts_land_on_their_coins() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("spotAssetCtxs")), Folded::Changed);
    assert_eq!(store.context("PURR/USDC").unwrap().mark(), dec("0.11107"));
    assert_eq!(store.context("@1").unwrap().mid(), Some(dec("11.9515")));
}

/// `spotMetaAndAssetCtxs`, put together from the recorded testnet meta and the recorded
/// contexts: the meta names each pair's quote, and each context lands on its coin.
#[test]
fn a_spot_read_lists_the_pairs_and_places_their_contexts() {
    let mut store = store();
    let recorded: serde_json::Value = read(&sent("spotAssetCtxs"));
    let meta: serde_json::Value = read(SPOT_META_TESTNET);
    let both: hl_spot::SpotMetaAndAssetCtxs = read(&serde_json::json!([meta, recorded["data"]]).to_string());
    assert_eq!(store.read(1_000, Read::Spot(&both)), Folded::Changed);
    assert_eq!(store.read(2_000, Read::Spot(&both)), Folded::Same);
    assert_eq!(store.surface("@1035"), Some(hl_account::Surface::Spot { quote: 0 }));
    assert_eq!(store.context("PURR/USDC").unwrap().mark(), dec("0.11107"));
    assert_eq!(store.context("PURR/USDC").unwrap().heard, 2_000);
}

/// Each dex's collateral is what `allPerpMetas` says: USDC for most, USDH (360) for `flx`, USDT0
/// (268) for `cash`.
#[test]
fn each_dex_s_collateral_is_what_its_meta_says() {
    let mut store = store();
    let _ = store.read(1_000, Read::Dexes(&dexes()));
    let collateral = store.collateral();
    assert_eq!(
        [collateral.of(""), collateral.of("xyz"), collateral.of("flx"), collateral.of("cash"), collateral.of("hyna")],
        [0, 0, 360, 268, 235]
    );
}

/// A frame built from the recorded book with one change, for what the venue did not send.
#[test]
fn a_book_for_another_coin_is_its_own() {
    let mut store = store();
    let _books = wanting(&mut store, [Interest::Book { coin: coin("BTC") }, Interest::Book { coin: coin("ETH") }]);
    let _ = store.apply(1_000, &frame("l2Book"));
    let eth = changed("l2Book", |json| json["data"]["coin"] = "ETH".into());
    let _ = store.apply(1_000, &eth);
    assert_eq!(store.best("ETH"), store.best("BTC"));
    assert!(store.book("SOL").is_none());
}
