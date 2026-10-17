//! `l2Book` as the venue answered it: `#14730` on mainnet, 2026-09-28, and an empty testnet book,
//! 2026-10-08.

use crate::utils::OutcomeSide;

use super::*;

const LIVE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/l2_book_14730_mainnet.json"));
const EMPTY_TESTNET: &str = r##"{"coin":"#231360","time":1791480981231,"levels":[[],[]]}"##;

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_live_book_reads_level_by_level_on_the_grid() {
    let book: Book = serde_json::from_str(LIVE).unwrap();
    assert_eq!(book.coin, Coin::new(1473, OutcomeSide::Yes));
    assert_eq!(book.time, 1_790_629_724_856);
    assert_eq!((book.bids.len(), book.asks.len()), (20, 20));
    assert_eq!(book.bids[0], Level { px: px("0.58495"), sz: Qty::new(286), n: 4 });
    assert_eq!(book.asks[19], Level { px: px("0.99975"), sz: Qty::new(12_000), n: 2 });
    assert_eq!(book.top(), Some((px("0.58495"), px("0.58497"))));
    assert!(book.bids.windows(2).all(|pair| pair[0].px > pair[1].px), "bids best first");
    assert!(book.asks.windows(2).all(|pair| pair[0].px < pair[1].px), "asks best first");
}

#[test]
fn an_empty_book_has_no_top() {
    let book: Book = serde_json::from_str(EMPTY_TESTNET).unwrap();
    assert_eq!(book.coin, Coin::new(23136, OutcomeSide::Yes));
    assert!(book.bids.is_empty() && book.asks.is_empty());
    assert_eq!(book.top(), None);
}

#[test]
fn a_book_off_the_grid_or_of_no_outcome_is_refused() {
    let fine = r##"{"coin":"#14730","time":1,"levels":[[{"px":"0.584951","sz":"1.0","n":1}],[]]}"##;
    assert!(serde_json::from_str::<Book>(fine).is_err());
    let perp = r#"{"coin":"BTC","time":1,"levels":[[],[]]}"#;
    assert!(serde_json::from_str::<Book>(perp).is_err());
}

#[test]
fn a_book_without_its_time_is_refused_not_read_as_1970() {
    let timeless = r##"{"coin":"#14730","levels":[[],[]]}"##;
    assert!(serde_json::from_str::<Book>(timeless).is_err());
}

#[test]
fn the_read_names_its_coin_with_a_hash() {
    assert_eq!(book_request(Coin::new(1473, OutcomeSide::Yes)), r##"{"type":"l2Book","coin":"#14730"}"##);
}
