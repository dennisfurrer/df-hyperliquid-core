//! BTC's book read on mainnet, 2026-10-08, at full precision, at five figures in steps of two and
//! at three figures (three levels a side kept); a recorded `l2Book` frame for ETH at five figures
//! and an empty outcome book the same day. Reads through a client are in `tests/mocked/markets`.

use super::*;

const BOOK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book.json"));
const BOOK_STEP_TWO: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book_five_figures_step_two.json"));
const BOOK_THREE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book_three_figures.json"));

const BUCKETED: &str = r#"{"coin":"ETH","time":1791480444439,"levels":[[{"px":"2410.2","sz":"655.1567","n":58}],[{"px":"2410.4","sz":"3.544","n":2}]],"spread":"0.1"}"#;
const EMPTY: &str = r##"{"coin":"#14720","time":1791480447083,"levels":[[],[]]}"##;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn book(text: &str) -> Book {
    serde_json::from_str(text).unwrap()
}

#[test]
fn a_book_reads_bids_then_asks_best_first() {
    let btc = book(BOOK);
    assert_eq!((btc.coin.as_str(), btc.time, btc.spread), ("BTC", 1_791_480_825_902, None));
    assert_eq!(btc.bids[0], Level { px: dec("80858"), sz: dec("23.7926"), n: 61 });
    assert!(btc.bids.windows(2).all(|pair| pair[0].px > pair[1].px), "bids highest first");
    assert!(btc.asks.windows(2).all(|pair| pair[0].px < pair[1].px), "asks lowest first");
    let cases = [
        (btc, (Some(dec("80858")), Some(dec("80859"))), Some(dec("80858.5")), None),
        (book(BOOK_STEP_TWO), (Some(dec("80858")), Some(dec("80860"))), Some(dec("80859")), Some(dec("1"))),
        (book(BOOK_THREE), (Some(dec("80800")), Some(dec("80900"))), Some(dec("80850")), Some(dec("1"))),
        (book(BUCKETED), (Some(dec("2410.2")), Some(dec("2410.4"))), Some(dec("2410.3")), Some(dec("0.1"))),
        (book(EMPTY), (None, None), None, None),
    ];
    for (read, touch, mid, spread) in cases {
        assert_eq!((read.touch(), read.mid(), read.spread), (touch, mid, spread), "{}", read.coin);
    }
    assert!(serde_json::from_str::<Book>(r#"{"coin":"BTC","levels":[[],[]]}"#).is_err(), "no time, no book");
}

#[test]
fn halfway_is_exact_and_a_place_finer_than_either_price() {
    for (bid, ask, mid) in [("0.58495", "0.58498", "0.584965"), ("100", "101", "100.5"), ("80858", "80860", "80859")] {
        assert_eq!(halfway(dec(bid), dec(ask)), Some(dec(mid)), "{bid} {ask}");
    }
}

#[test]
fn a_book_is_asked_for_only_at_a_precision_the_venue_takes() {
    let five = Precision::figures(5).unwrap();
    let cases = [
        (Some(Precision::FULL), r#"{"type":"l2Book","coin":"BTC"}"#),
        (Precision::figures(3), r#"{"type":"l2Book","coin":"BTC","nSigFigs":3}"#),
        (five.step(2), r#"{"type":"l2Book","coin":"BTC","nSigFigs":5,"mantissa":2}"#),
    ];
    for (precision, body) in cases {
        assert_eq!(l2_book_request("BTC", precision.unwrap()), body);
    }
    for figures in 2..=5 {
        assert!(Precision::figures(figures).is_some(), "{figures} figures");
    }
    for refused in [0, 1, 6] {
        assert_eq!(Precision::figures(refused), None, "{refused} figures");
    }
    for step in [1, 2, 5] {
        assert!(five.step(step).is_some(), "a step of {step}");
    }
    assert_eq!(five.step(3), None);
    assert_eq!(Precision::figures(3).and_then(|three| three.step(2)), None, "a step only at five figures");
}
