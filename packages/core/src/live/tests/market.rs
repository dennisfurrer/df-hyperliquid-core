//! The market frames as recorded on mainnet: books, prints, the best bid and offer, mids,
//! candles and contexts, every price and size exact; and the `l2` book's snapshot and diffs, each
//! recorded diff applied in order.

use crate::live::fixtures::{address, compressed, dec, inflated, level, mainnet, read, testnet};
use crate::live::{BadDiff, Frame, L2, L2Diff, L2Level};
use crate::markets::{Interval, Side};

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

/// A perp's context and a spot pair's or an outcome side's come on their own channels; every
/// dex's, one dex's and spot's read alike. Outcome changes and the fast contexts are kept as sent.
#[test]
fn every_context_reads_exactly_on_its_own_channel() {
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

    let Frame::OutcomeMetaUpdates(changes) = read(&testnet("outcomeMetaUpdates")) else { panic!("changes") };
    assert_eq!(changes.compact(), r#"[{"outcomeSettled":23205}]"#);
    let Frame::FastAssetCtxs(payload) = read(&mainnet("fastAssetCtxs")) else { panic!("fast contexts") };
    assert!(payload.starts_with("bVjLruM4Dv2X9LYg6C35"));
    assert!(payload.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')), "base64");
}

fn base64(payload: &str) -> bool {
    payload.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
}

/// `l2` opens with a snapshot as `l2Book` sends it, then sends each diff compressed. `pac` and `sac`
/// are compressed whole and kept as sent.
#[test]
fn l2_opens_with_a_snapshot_then_sends_compressed_diffs_and_pac_and_sac_are_kept_as_sent() {
    let Frame::L2(L2::Snapshot(book)) = read(&compressed("l2")) else { panic!("a snapshot") };
    assert_eq!((book.coin.as_str(), book.time, book.spread), ("BTC", 1_795_489_962_741, None));
    assert_eq!((book.bids.len(), book.asks.len()), (20, 20));
    assert_eq!(book.bids[0], level("59963.0", "2.22088", 11));
    let Frame::L2(L2::Snapshot(grouped)) = read(&compressed("l2.sigFigs")) else { panic!("a snapshot") };
    assert_eq!((grouped.coin.as_str(), grouped.bids[0]), ("ETH", level("1680.0", "11205.2825", 504)));

    for name in ["l2.diff", "l2.diff.sigFigs"] {
        let Frame::L2(L2::Compressed(payload)) = read(&compressed(name)) else { panic!("{name} is compressed") };
        assert!(base64(&payload), "{name}");
    }
    let Frame::Pac(pac) = read(&compressed("pac")) else { panic!("pac") };
    let Frame::Sac(sac) = read(&compressed("sac")) else { panic!("sac") };
    assert!(pac.starts_with("rV3LjiU3cv2X") && base64(&pac));
    assert!(base64(&sac) && !sac.is_empty());
}

/// A diff sent as it is reads as one inflated from `c`; the docs' `u`. A grouped book's diff
/// carries its spread.
#[test]
fn a_diff_reads_from_its_inflated_text_and_as_sent_whole() {
    let first: L2Diff = serde_json::from_str(&inflated()[0]).unwrap();
    assert_eq!((first.coin.as_str(), first.time), ("BTC", 1_795_489_963_277));
    assert_eq!((first.bids.len(), first.asks.len()), (19, 14));
    assert_eq!(first.bids[0], L2Level { px: dec("59963.0"), sz: dec("2.47796") });
    assert_eq!(
        (first.removed_bids.as_slice(), first.removed_asks.as_slice(), first.spread),
        (&[19, 18, 1][..], &[][..], None)
    );
    let whole = format!(r#"{{"channel":"l2","data":{{"u":{}}}}}"#, inflated()[0]);
    assert_eq!(read(&whole), Frame::L2(L2::Diff(first)));
    let grouped: L2Diff =
        serde_json::from_str(r#"{"c":"ETH","t":1,"l":[[{"p":"1680.0","s":"11555.0687"}],[]],"r":[[],[]],"s":"0.1"}"#)
            .unwrap();
    assert_eq!(grouped.spread, Some(dec("0.1")));
}

/// The ten diffs after the BTC snapshot, applied in order, keep twenty levels a side, best first,
/// with the bid under the ask; each changed level has no order count.
#[test]
fn the_recorded_diffs_apply_in_order_and_keep_the_book_whole_and_uncrossed() {
    let Frame::L2(L2::Snapshot(mut book)) = read(&compressed("l2")) else { panic!("a snapshot") };
    let diffs: Vec<L2Diff> = inflated().iter().map(|text| serde_json::from_str(text).unwrap()).collect();
    assert_eq!(diffs.len(), 10);
    for diff in &diffs {
        diff.apply(&mut book).unwrap();
        assert_eq!((book.bids.len(), book.asks.len(), book.time), (20, 20, diff.time));
        assert!(book.bids.windows(2).all(|pair| pair[0].px > pair[1].px), "bids fall away from the best");
        assert!(book.asks.windows(2).all(|pair| pair[0].px < pair[1].px), "asks rise away from the best");
        let (bid, ask) = book.touch();
        assert!(bid < ask, "{bid:?} {ask:?}");
        for level in diff.bids.iter().chain(&diff.asks) {
            let side = if diff.bids.contains(level) { &book.bids } else { &book.asks };
            assert!(side.contains(&crate::markets::Level { px: level.px, sz: level.sz, n: 0 }), "{level:?}");
        }
    }
    assert_eq!(book.touch(), (Some(dec("59832.0")), Some(dec("59833.0"))));
}

/// A diff for another coin, or one that removes a level the book does not have, is refused and
/// leaves the book as it was.
#[test]
fn a_diff_that_does_not_fit_the_book_is_refused_and_changes_nothing() {
    let Frame::L2(L2::Snapshot(book)) = read(&compressed("l2")) else { panic!("a snapshot") };
    let mut diff: L2Diff = serde_json::from_str(&inflated()[0]).unwrap();
    let mut kept = book.clone();
    diff.coin = "ETH".to_owned();
    assert_eq!(diff.apply(&mut kept), Err(BadDiff::Coin { book: "BTC".to_owned(), diff: "ETH".to_owned() }));
    diff.coin = "BTC".to_owned();
    diff.removed_asks = vec![20];
    assert_eq!(diff.apply(&mut kept), Err(BadDiff::Index { index: 20, levels: 20 }));
    assert_eq!(kept, book);
}
