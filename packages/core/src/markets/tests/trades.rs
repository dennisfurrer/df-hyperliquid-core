//! Three BTC prints read on mainnet, 2026-10-08, and a recorded `trades` frame row the same day.
//! Reads through a client are in `tests/mocked/markets`.

use super::*;

const TRADES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/recent_trades.json"));
const SENT: &str = r#"{"coin":"BTC","side":"A","px":"80600.0","sz":"0.16","time":1791480516231,"hash":"0x33062daa8fe10fcd347f0446200d470206e300902ae42e9fd6ced8fd4ee4e9b7","tid":717978735161105,"users":["0x654086857e1fad6dcf05cf6695cce51ea3984268","0xa168f51d1ec7f7a4cd38e9b6a461b05f64acfac8"]}"#;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn a_print_names_the_aggressor_then_its_buyer_and_its_seller() {
    let trades: Vec<Trade> = serde_json::from_str(TRADES).unwrap();
    let first = &trades[0];
    assert_eq!(
        (first.coin.as_str(), first.side, first.px, first.sz),
        ("BTC", Side::Buy, Dec::from_int(80_839), Dec::parse("0.03885").unwrap())
    );
    assert_eq!((first.time, first.tid), (1_791_480_719_939, 739_564_670_599_029));
    assert_eq!(first.hash, "0x27597855ce050dd328d30446201873020639003b69082ca5cb2223a88d08e7bd");
    assert_eq!(
        first.users(),
        [address("0xc861d75087be18a9f46c1dee457bb0e29e31ddbd"), address("0x9266865bb6afb4c4f618544dd3b8c970f17aa664")]
    );
    let sent: Trade = serde_json::from_str(SENT).unwrap();
    assert_eq!(sent.side, Side::Sell, "a sell took");
    assert_eq!(
        (sent.buyer, sent.seller),
        (address("0x654086857e1fad6dcf05cf6695cce51ea3984268"), address("0xa168f51d1ec7f7a4cd38e9b6a461b05f64acfac8")),
        "the buyer first, whichever side took"
    );
}
