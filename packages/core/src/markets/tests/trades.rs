//! A recorded `recentTrades` row and a recorded `trades` frame row (BTC, mainnet, 2026-10-08), and
//! three BTC prints read live the same day.

use super::*;
use crate::trade::tests_support::{Recorded, block_on};

const TRADES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/recent_trades.json"));

const READ: &str = r#"{"coin":"BTC","side":"B","px":"80839.0","sz":"0.03885","time":1791480719939,"hash":"0x27597855ce050dd328d30446201873020639003b69082ca5cb2223a88d08e7bd","tid":739564670599029,"users":["0xc861d75087be18a9f46c1dee457bb0e29e31ddbd","0x9266865bb6afb4c4f618544dd3b8c970f17aa664"]}"#;
const SENT: &str = r#"{"coin":"BTC","side":"A","px":"80600.0","sz":"0.16","time":1791480516231,"hash":"0x33062daa8fe10fcd347f0446200d470206e300902ae42e9fd6ced8fd4ee4e9b7","tid":717978735161105,"users":["0x654086857e1fad6dcf05cf6695cce51ea3984268","0xa168f51d1ec7f7a4cd38e9b6a461b05f64acfac8"]}"#;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn a_print_names_its_buyer_then_its_seller_whichever_side_took() {
    let read: Trade = serde_json::from_str(READ).unwrap();
    assert_eq!((read.side, read.px, read.tid), (Side::Buy, Dec::from_int(80_839), 739_564_670_599_029));
    assert_eq!(read.buyer, address("0xc861d75087be18a9f46c1dee457bb0e29e31ddbd"));
    let sent: Trade = serde_json::from_str(SENT).unwrap();
    assert_eq!(sent.side, Side::Sell);
    assert_eq!(
        sent.users(),
        [address("0x654086857e1fad6dcf05cf6695cce51ea3984268"), address("0xa168f51d1ec7f7a4cd38e9b6a461b05f64acfac8")]
    );
}

#[test]
fn a_print_names_the_aggressor_and_both_sides() {
    assert_eq!(recent_trades_request("BTC"), r#"{"type":"recentTrades","coin":"BTC"}"#);
    let trades: Vec<Trade> = serde_json::from_str(TRADES).unwrap();
    let first = &trades[0];
    assert_eq!(
        (first.side, first.px, first.sz, first.tid),
        (Side::Buy, Dec::from_int(80_839), Dec::parse("0.03885").unwrap(), 739_564_670_599_029)
    );
    assert_eq!(first.users()[0], address("0xc861d75087be18a9f46c1dee457bb0e29e31ddbd"));
    assert_eq!(trades[1].side, Side::Sell);
    let transport = Recorded::answering(TRADES);
    assert_eq!(block_on(recent_trades(&transport, "BTC")).unwrap().len(), 3);
}
