//! A recorded `l2Book` answer (BTC) and recorded `l2Book` frames (ETH at five figures, and an
//! empty outcome book), all mainnet on 2026-10-08, one level a side kept; and BTC's book read live
//! the same day at full precision, at five figures in steps of two and at three figures (three
//! levels a side kept).

use super::*;
use crate::trade::tests_support::{Recorded, block_on};

const BOOK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book.json"));
const BOOK_STEP_TWO: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book_five_figures_step_two.json"));
const BOOK_THREE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book_three_figures.json"));

const READ: &str = r#"{"coin":"BTC","time":1791480825902,"levels":[[{"px":"80858.0","sz":"23.7926","n":61}],[{"px":"80859.0","sz":"0.94647","n":5}]]}"#;
const BUCKETED: &str = r#"{"coin":"ETH","time":1791480444439,"levels":[[{"px":"2410.2","sz":"655.1567","n":58}],[{"px":"2410.4","sz":"3.544","n":2}]],"spread":"0.1"}"#;
const EMPTY: &str = r##"{"coin":"#14720","time":1791480447083,"levels":[[],[]]}"##;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_book_reads_bids_then_asks_best_first() {
    let book: Book = serde_json::from_str(READ).unwrap();
    assert_eq!((book.coin.as_str(), book.time, book.spread), ("BTC", 1_791_480_825_902, None));
    assert_eq!(book.bids, [Level { px: dec("80858"), sz: dec("23.7926"), n: 61 }]);
    assert_eq!(book.touch(), (Some(dec("80858")), Some(dec("80859"))));
    assert_eq!(book.mid(), Some(dec("80858.5")), "the midpoint, exactly");
    let bucketed: Book = serde_json::from_str(BUCKETED).unwrap();
    assert_eq!((bucketed.spread, bucketed.mid()), (Some(dec("0.1")), Some(dec("2410.3"))));
}

#[test]
fn an_empty_side_has_no_touch_and_no_mid() {
    let book: Book = serde_json::from_str(EMPTY).unwrap();
    assert_eq!(book.touch(), (None, None));
    assert_eq!(book.mid(), None);
}

#[test]
fn a_book_without_its_time_is_refused() {
    assert!(serde_json::from_str::<Book>(r#"{"coin":"BTC","levels":[[],[]]}"#).is_err());
}

#[test]
fn halfway_is_a_place_finer_than_either_price() {
    assert_eq!(halfway(dec("0.58495"), dec("0.58498")), Some(dec("0.584965")));
    assert_eq!(halfway(dec("100"), dec("101")), Some(dec("100.5")));
}

#[test]
fn a_book_is_asked_for_at_a_precision_the_venue_takes() {
    assert_eq!(l2_book_request("BTC", Precision::FULL), r#"{"type":"l2Book","coin":"BTC"}"#);
    let three = Precision::figures(3).unwrap();
    assert_eq!(l2_book_request("BTC", three), r#"{"type":"l2Book","coin":"BTC","nSigFigs":3}"#);
    let step_two = Precision::figures(5).and_then(|five| five.step(2)).unwrap();
    assert_eq!(l2_book_request("BTC", step_two), r#"{"type":"l2Book","coin":"BTC","nSigFigs":5,"mantissa":2}"#);
    for refused in [0, 1, 6] {
        assert_eq!(Precision::figures(refused), None, "{refused} figures");
    }
    assert_eq!(three.step(2), None, "a step only at five figures");
    for step in [1, 2, 5] {
        assert!(Precision::figures(5).and_then(|five| five.step(step)).is_some(), "{step}");
    }
    assert_eq!(Precision::figures(5).and_then(|five| five.step(3)), None);
    let transport = Recorded::answering(BOOK);
    assert!(block_on(l2_book(&transport, "BTC", Precision::FULL)).is_ok());
    assert_eq!(transport.sent(), [("/info".to_owned(), l2_book_request("BTC", Precision::FULL))]);
}

#[test]
fn a_book_reads_both_sides_best_first() {
    let book: Book = serde_json::from_str(BOOK).unwrap();
    assert_eq!((book.coin.as_str(), book.time, book.spread), ("BTC", 1_791_480_825_902, None));
    assert_eq!(book.bids[0], Level { px: dec("80858"), sz: dec("23.7926"), n: 61 });
    assert_eq!(book.asks[0].px, dec("80859"));
    assert_eq!(book.touch(), (Some(dec("80858")), Some(dec("80859"))));
    assert_eq!(book.mid(), Some(dec("80858.5")), "the midpoint, exactly");
    let stepped: Book = serde_json::from_str(BOOK_STEP_TWO).unwrap();
    assert_eq!((stepped.asks[0].px, stepped.spread), (dec("80860"), Some(dec("1"))));
    let three: Book = serde_json::from_str(BOOK_THREE).unwrap();
    assert_eq!((three.bids[0].px, three.bids[0].n), (dec("80800"), 383));
}
