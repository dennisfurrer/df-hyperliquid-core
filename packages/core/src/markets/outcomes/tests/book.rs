//! `l2Book` as the venue answered it: `#14730` on mainnet, 2026-09-28, and an empty testnet book,
//! 2026-10-08. Reads through a client are in `tests/mocked/markets`.

use crate::utils::OutcomeSide;

use super::*;

const LIVE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/l2_book_14730_mainnet.json"));
const EMPTY_TESTNET: &str = r##"{"coin":"#231360","time":1791480981231,"levels":[[],[]]}"##;

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_side_s_book_reads_level_by_level_on_the_grid() {
    let book: Book = serde_json::from_str(LIVE).unwrap();
    assert_eq!((book.coin, book.time), (Coin::new(1473, OutcomeSide::Yes), 1_790_629_724_856));
    assert_eq!((book.bids.len(), book.asks.len()), (20, 20));
    assert_eq!(book.bids[0], Level { px: px("0.58495"), sz: Qty::new(286), n: 4 });
    assert_eq!(book.asks[19], Level { px: px("0.99975"), sz: Qty::new(12_000), n: 2 });
    assert_eq!(book.top(), Some((px("0.58495"), px("0.58497"))));
    assert!(book.bids.windows(2).all(|pair| pair[0].px > pair[1].px), "bids best first");
    assert!(book.asks.windows(2).all(|pair| pair[0].px < pair[1].px), "asks best first");
    let read: crate::markets::Book = serde_json::from_str(LIVE).unwrap();
    assert_eq!(Book::try_from(read), Ok(book), "the venue's book, onto the grid");
    let empty: Book = serde_json::from_str(EMPTY_TESTNET).unwrap();
    assert_eq!((empty.coin, empty.bids.len(), empty.asks.len()), (Coin::new(23136, OutcomeSide::Yes), 0, 0));
    assert_eq!(empty.top(), None);
}

#[test]
fn a_book_off_the_grid_of_no_outcome_or_without_its_time_is_refused() {
    let refused = [
        (
            r##"{"coin":"#14730","time":1,"levels":[[{"px":"0.584951","sz":"1.0","n":1}],[]]}"##,
            "finer than the 0.00001 tick",
        ),
        (r##"{"coin":"#14730","time":1,"levels":[[],[{"px":"0.5","sz":"1.5","n":1}]]}"##, "fractional"),
        (r#"{"coin":"BTC","time":1,"levels":[[],[]]}"#, "is not an outcome side"),
        (r##"{"coin":"#14730","levels":[[],[]]}"##, "missing field `time`"),
    ];
    for (text, why) in refused {
        let error = serde_json::from_str::<Book>(text).unwrap_err().to_string();
        assert!(error.contains(why), "{text}: {error}");
    }
}
