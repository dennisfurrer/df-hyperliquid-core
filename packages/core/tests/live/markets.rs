//! What is listed and what it trades at, read from mainnet.

use df_hyperliquid_core::markets::{self, Dex, Precision, perps};

use crate::tap::{reader, same_shape};

#[tokio::test]
#[ignore = "mainnet"]
async fn perp_meta_lists_btc() {
    let client = reader();
    let meta = perps::meta(&client, Dex::Main).await.unwrap().unwrap();
    assert!(meta.find(0, "BTC").is_some());
    same_shape(&client, "perps/meta_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_btc_book_has_both_sides() {
    let client = reader();
    let book = markets::book::l2_book(&client, "BTC", Precision::FULL).await.unwrap();
    let (bid, ask) = book.touch();
    assert!(bid.unwrap() < ask.unwrap());
    same_shape(&client, "trade/l2_book.json");
}
