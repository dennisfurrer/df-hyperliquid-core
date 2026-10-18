//! Market data into the store: books, mids, candles and contexts, and the metas that place
//! contexts. A book or candles are kept only while an interest wants them.

use crate::client::Network;
use crate::live::Frame;
use crate::live::store::fixtures::{
    ALL_MIDS, CANDLE_SNAPSHOT, L2_BOOK, PARA, PERP_META, SPOT_META_TESTNET, changed, dec, dexes, frame, read, sent,
};
use crate::live::store::{Context, Folded, Handle, Interest, Read, Store};
use crate::markets::{Book, Candle, Interval, Level, Mids};
use crate::utils::Dec;

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

/// The recorded BTC book (mainnet, 2026-10-08): its touch, and halfway between, exactly. Told
/// again unchanged it is heard, not changed: staleness reads `heard`. Each coin's book is its
/// own, and goes with its last interest.
#[test]
fn a_wanted_book_gives_the_touch_and_the_mid_between() {
    let mut store = store();
    let mut held = wanting(&mut store, [Interest::Book { coin: coin("BTC") }, Interest::Book { coin: coin("ETH") }]);
    assert_eq!(store.apply(1_000, &frame("l2Book")), Folded::Changed);
    assert_eq!(store.best("BTC"), (Some(level("80459.0", "16.87185", 49)), Some(level("80460.0", "4.21967", 21))));
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")));
    let book = store.book("BTC").unwrap();
    assert_eq!((book.time, book.bids.len(), book.heard, book.changed), (1791480444439, 20, 1_000, 1_000));
    assert_eq!(store.apply(4_000, &frame("l2Book")), Folded::Same);
    let book = store.book("BTC").unwrap();
    assert_eq!((book.heard, book.changed, book.age(10_000)), (4_000, 1_000, 6_000));

    // A frame built from the recorded book with one change, for what the venue did not send.
    let eth = changed("l2Book", |json| json["data"]["coin"] = "ETH".into());
    assert_eq!(store.apply(1_000, &eth), Folded::Changed);
    assert_eq!(store.best("ETH"), store.best("BTC"));
    assert!(store.book("SOL").is_none());
    store.release(held.remove(0));
    assert!(store.book("BTC").is_none());
    assert_eq!(store.best("BTC"), (None, None));
    assert!(store.book("ETH").is_some());
}

/// A book no interest wants is not kept. A book carries no sequence, so its own time orders it:
/// the REST read (06:53:45) is newer than the socket's frame (06:47:24), and the frame arriving
/// after it is older news. A grouped book (`nSigFigs`, sent with a spread) is not the full book
/// the store asks for.
#[test]
fn an_unwanted_an_older_or_a_grouped_book_is_dropped() {
    let mut store = store();
    let book: Book = read(L2_BOOK);
    assert_eq!(store.apply(1_000, &frame("l2Book")), Folded::Dropped);
    assert_eq!(store.read(1_000, Read::Book(&book)), Folded::Dropped);
    assert!(store.book("BTC").is_none());

    let _books = wanting(&mut store, [Interest::Book { coin: coin("BTC") }, Interest::Book { coin: coin("ETH") }]);
    assert_eq!(store.read(1_000, Read::Book(&book)), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("l2Book")), Folded::Dropped);
    assert_eq!(store.book("BTC").unwrap().time, 1791480825902);
    assert_eq!(store.best("BTC").0, Some(level("80858.0", "23.7926", 61)));
    assert_eq!(store.apply(1_000, &frame("l2Book.sigFigs")), Folded::Dropped);
    assert!(store.book("ETH").is_none());
}

/// `allMids` covers every coin: the first dex's carries spot pairs and outcome sides too, and a
/// HIP-3 dex's its own; mids are kept whether or not an interest asked. A held book is sent on
/// every change, so its mid wins over the list; an empty book (an outcome side no one quotes)
/// falls back to it.
#[test]
fn the_venue_s_mids_cover_every_coin_and_a_held_book_s_mid_wins() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("allMids")), Folded::Changed);
    assert_eq!(store.apply(1_000, &frame("allMids.xyz")), Folded::Changed);
    assert_eq!(store.apply(1_500, &frame("allMids.xyz")), Folded::Same);
    for (coin, mid) in [
        ("BTC", Some("80474.5")),
        ("@107", Some("82.897")),
        ("#14720", Some("0.5")),
        ("xyz:AAPL", Some("338.095")),
        ("xyz:NOPE", None),
    ] {
        assert_eq!(store.mid(coin), mid.map(dec), "{coin}");
    }
    assert_eq!(store.mids("xyz").unwrap().heard, 1_500);

    let _books = wanting(&mut store, [Interest::Book { coin: coin("BTC") }, Interest::Book { coin: coin("#14720") }]);
    let _ = store.apply(1_000, &frame("l2Book"));
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")));
    assert_eq!(store.apply(1_000, &frame("l2Book.empty")), Folded::Changed);
    assert_eq!((store.best("#14720"), store.mid("#14720")), ((None, None), Some(dec("0.5"))));

    let mids: Mids = read(ALL_MIDS);
    assert_eq!(store.read(2_000, Read::Mids { dex: "", mids: &mids }), Folded::Changed);
    assert_eq!(store.mid("ETH"), Some(dec("2422.75")), "a read replaces the dex's mids");
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")), "and the held book's still wins");
    assert_eq!(store.mid("SOL"), None, "the read named no SOL");
}

/// The recorded 1m candle, and the recorded hour candles read, each kept at its own interval.
/// History read later goes under what the socket already sent; the live candle wins.
#[test]
fn candles_are_kept_a_market_and_interval_each_with_history_under_the_live_one() {
    let mut store = store();
    let _candles = wanting(
        &mut store,
        [
            Interest::Candles { coin: coin("BTC"), interval: Interval::OneMinute },
            Interest::Candles { coin: coin("BTC"), interval: Interval::OneHour },
        ],
    );
    assert_eq!(store.apply(1_000, &frame("candle")), Folded::Changed);
    let history: Vec<Candle> = read(CANDLE_SNAPSHOT);
    assert_eq!(store.read(1_000, Read::Candles(&history)), Folded::Changed);
    let Frame::Candle(minute) = frame("candle") else { panic!("a candle") };
    let live = Candle {
        interval: Interval::OneHour,
        open_time: 1791478800000,
        close_time: 1791482399999,
        close: dec("80999.0"),
        ..minute
    };
    assert_eq!(store.apply(2_000, &Frame::Candle(live)), Folded::Changed, "the hour forming moved");
    assert_eq!(store.read(3_000, Read::Candles(&history)), Folded::Same, "read again, it goes under the live one");
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
            (1791478800000, dec("80999.0")),
        ]
    );
    assert!(store.candles("BTC", Interval::FiveMinutes).is_none());
    assert_eq!(store.apply(4_000, &changed("candle", |json| json["data"]["s"] = "ETH".into())), Folded::Dropped);
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
    assert_eq!((hype.mark_px, hype.circulating_supply), (dec("82.858"), dec("302175679.3606832623")));
    let side = store.context("#14720").unwrap();
    assert_eq!((side.mark(), side.mid(), side.heard), (dec("0.5"), None, 1_000));
    assert!(store.context("ETH").is_none());
    assert_eq!(store.mid("#14720"), None, "no book, no list, and a context with no mid");
    assert_eq!(store.mid("@107"), Some(dec("82.848")), "the context's mid when nothing else says");
}

/// `allDexsAssetCtxs` sends each dex's contexts in its universe's order, which `perpDexs` and
/// `allPerpMetas` give. Recorded the same moment, the context it places on BTC is the one
/// `activeAssetCtx` sent for BTC. One dex's contexts (`assetCtxs`), one dex's read with its meta,
/// and the first dex's `metaAndAssetCtxs` (the recorded meta with the recorded contexts of its
/// first two perps) land the same way. Each dex's collateral is what its meta says: USDC for
/// most, USDH (360) for `flx`, USDT0 (268) for `cash`.
#[test]
fn every_dex_s_contexts_land_on_their_coins_in_its_universe_s_order() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("allDexsAssetCtxs")), Folded::Dropped, "no universe yet to place them");
    let dexes = dexes();
    assert_eq!(store.read(1_000, Read::Dexes(&dexes)), Folded::Changed);
    assert_eq!(store.read(1_000, Read::Dexes(&dexes)), Folded::Same);
    let collateral = store.collateral();
    assert_eq!(
        [collateral.of(""), collateral.of("xyz"), collateral.of("flx"), collateral.of("cash"), collateral.of("hyna")],
        [0, 0, 360, 268, 235]
    );
    assert_eq!(store.apply(2_000, &frame("allDexsAssetCtxs")), Folded::Changed);
    let Some(Context::Perp(placed)) = store.context("BTC").map(|ctx| ctx.value) else { panic!("a perp") };
    let placed = placed.clone();
    assert_eq!(store.apply(3_000, &frame("activeAssetCtx")), Folded::Same);
    assert_eq!(store.context("BTC").unwrap().value, Context::Perp(&placed));
    assert_eq!(store.context("BTC").unwrap().changed, 2_000, "the same context, told again");
    assert_eq!(store.context("ETH").unwrap().mark(), dec("2410.3"));
    let Some(Context::Perp(abcd)) = store.context("abcd:USA500").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!((abcd.mark_px, abcd.mid_px), (dec("6980.0"), None));

    assert_eq!(store.apply(1_000, &frame("assetCtxs")), Folded::Changed);
    assert_eq!(store.context("xyz:XYZ100").unwrap().mark(), dec("30567.0"));
    assert_eq!(store.context("xyz:TSLA").unwrap().mark(), dec("368.56"));
    let para: crate::markets::perps::MetaAndAssetCtxs = read(PARA);
    assert_eq!(store.read(2_000, Read::Dex { dex: "para", read: &para }), Folded::Changed);
    let Some(Context::Perp(total)) = store.context("para:TOTAL2").map(|ctx| ctx.value) else { panic!("a perp") };
    assert_eq!((total.mark_px, total.mid_px), (dec("1179.574"), Some(dec("1179.65"))));

    let mut store = self::store();
    let recorded: serde_json::Value = read(&sent("allDexsAssetCtxs"));
    let meta: serde_json::Value = read(PERP_META);
    let both: crate::markets::perps::MetaAndAssetCtxs =
        read(&serde_json::json!([meta, recorded["data"]["ctxs"][0][1]]).to_string());
    assert_eq!(both.ctxs.len(), 2);
    assert_eq!(store.read(1_000, Read::Perps(&both)), Folded::Changed);
    assert_eq!(store.context("ETH").unwrap().mark(), dec("2410.3"));
    assert!(store.context("ATOM").is_none(), "a perp with no context read has none");
    assert_eq!(store.collateral().of(""), 0);
}

/// Spot contexts land on their coins from `spotAssetCtxs`, and from `spotMetaAndAssetCtxs` (the
/// recorded testnet meta with the recorded contexts), whose meta names each pair's quote.
#[test]
fn spot_contexts_land_on_their_coins_and_a_spot_read_lists_the_pairs() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("spotAssetCtxs")), Folded::Changed);
    assert_eq!(store.context("PURR/USDC").unwrap().mark(), dec("0.11107"));
    assert_eq!(store.context("@1").unwrap().mid(), Some(dec("11.9515")));

    let recorded: serde_json::Value = read(&sent("spotAssetCtxs"));
    let meta: serde_json::Value = read(SPOT_META_TESTNET);
    let both: crate::markets::spot::SpotMetaAndAssetCtxs =
        read(&serde_json::json!([meta, recorded["data"]]).to_string());
    assert_eq!(store.read(1_500, Read::Spot(&both)), Folded::Changed, "the pairs are new");
    assert_eq!(store.read(2_000, Read::Spot(&both)), Folded::Same);
    assert_eq!(store.surface("@1035"), Some(crate::account::Surface::Spot { quote: 0 }));
    assert_eq!(store.context("PURR/USDC").unwrap().mark(), dec("0.11107"));
    assert_eq!(store.context("PURR/USDC").unwrap().heard, 2_000);
}
