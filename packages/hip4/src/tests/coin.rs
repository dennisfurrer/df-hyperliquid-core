//! hip4-tui's outcome order and cancel vectors (`crypto/src/tests/action.rs`,
//! checked against the venue's own client and a separate TypeScript
//! implementation), and the whole posted body its exchange signed with the
//! published key `0x..01` (`net/src/tests/exchange.rs`). Mainnet.

use hl_core::{Address, L1Action, LocalKey, hex, recover};
use hl_trade::{Cancel, Order, PlaceOrder};

use super::*;

const NONCE: u64 = 1_712_345_678_000;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// `#5160`: outcome 516, Yes.
fn coin() -> Coin {
    Coin::new(516, OutcomeSide::Yes)
}

fn canonical() -> PlaceOrder {
    PlaceOrder::one(Order::market(coin().asset().unwrap(), Side::Buy, dec("0.35"), dec("50")))
}

#[test]
fn the_order_action_hashes_to_the_verified_value() {
    let action = canonical();
    assert_eq!(
        action.wire(),
        r#"{"type":"order","orders":[{"a":100005160,"b":true,"p":"0.35","s":"50","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
    let packed = action.msgpack();
    assert_eq!(packed.len(), 87);
    assert_eq!(
        hex::encode(&packed),
        concat!(
            "83a474797065a56f72646572a66f726465727391",
            "86a161ce05f5f528a162c3a170a4302e3335a173a23530a172c2",
            "a174",
            "81a56c696d697481a374",
            "6966ae46726f6e74656e644d61726b6574a86772",
            "6f7570696e67a26e61",
        )
    );
    assert_eq!(
        hex::encode(&action.connection_id(NONCE, None)),
        "b1326e8fc384f433fc462a79ca16e7c28f7b778da1d659c9f0873add8f8e498d"
    );
}

/// The exchange's own test, `a_place_becomes_the_exact_post_body_the_venue_is_signed_against`.
#[test]
fn the_order_s_body_is_the_one_the_venue_is_signed_against() {
    let key = LocalKey::from_hex("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let published: Address = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf".parse().unwrap();
    assert_eq!(key.address(), published);
    let action = canonical();
    let signature = key.sign_digest(&action.digest(NONCE, None)).unwrap();
    assert_eq!(recover(&action.digest(NONCE, None), &signature), Ok(published));
    assert_eq!(
        action.body(NONCE, &signature, None),
        concat!(
            r#"{"action":{"type":"order","orders":[{"a":100005160,"b":true,"p":"0.35","#,
            r#""s":"50","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"#,
            r#""grouping":"na"},"nonce":1712345678000,"#,
            r#""signature":{"r":"0x80af9e88b62b9fcff44048c4b7ca9993264a30d2826919872c0ca6433fb6b15d","#,
            r#""s":"0x0a09eafcb870e3411f6ea8c3ba8310bce6e012ba29c2fb0acf10531fdba08cfc","v":27},"#,
            r#""vaultAddress":null}"#,
        )
    );
}

#[test]
fn the_cancel_action_hashes_to_the_verified_value() {
    let action = Cancel::one(coin().asset().unwrap(), 123_456_789);
    let expected: Vec<u8> = [
        &[0x82][..],
        &[0xa4],
        b"type",
        &[0xa6],
        b"cancel",
        &[0xa7],
        b"cancels",
        &[0x91],
        &[0x82],
        &[0xa1],
        b"a",
        &[0xce, 0x05, 0xf5, 0xf5, 0x28],
        &[0xa1],
        b"o",
        &[0xce, 0x07, 0x5b, 0xcd, 0x15],
    ]
    .concat();
    assert_eq!(action.msgpack(), expected);
    assert_eq!(
        hex::encode(&action.connection_id(NONCE, None)),
        "cb2ae13537e67a6dccbcdb398f2cd008573d1bfd94dd42ac07f231321d27533a"
    );
}

/// hip4-tui's `a_market_order_is_marketable_at_the_edge_of_the_range_and_never_gtc`.
#[test]
fn a_market_order_is_frontend_market_at_the_edge_of_the_range_and_never_gtc() {
    let buy = coin().market(Side::Buy, dec("50")).unwrap();
    assert_eq!((buy.price(), buy.tif()), (MAX_PRICE, Some(Tif::FrontendMarket)));
    let sell = coin().market(Side::Sell, dec("50")).unwrap();
    assert_eq!((sell.price(), sell.tif()), (MIN_PRICE, Some(Tif::FrontendMarket)));
    assert!(buy.is_market() && sell.is_market());
    let posted = PlaceOrder::one(sell).wire();
    assert!(posted.contains(r#""p":"0.00001""#) && posted.contains(r#""tif":"FrontendMarket""#), "{posted}");
    assert!(!posted.contains("Gtc"), "{posted}");
    let resting = coin().limit(Side::Buy, dec("0.35"), dec("50"), Tif::Gtc).unwrap();
    assert!(!resting.is_market());
}

#[test]
fn an_outcome_price_is_a_tick_of_a_hundred_thousandth_never_against_the_trader() {
    assert_eq!(outcome_price(dec("0.123456"), Side::Buy), Some(dec("0.12345")));
    assert_eq!(outcome_price(dec("0.123451"), Side::Sell), Some(dec("0.12346")));
    assert_eq!(outcome_price(dec("0.999995"), Side::Buy), Some(MAX_PRICE));
    assert_eq!(outcome_price(dec("0.999991"), Side::Sell), None, "rounds up to 1, past the range");
    assert_eq!(outcome_price(dec("0.000004"), Side::Buy), None);
    assert_eq!(outcome_price(dec("1"), Side::Buy), None);
    assert_eq!(outcome_size(dec("50.9")), Some(dec("50")), "whole contracts, toward zero");
    assert_eq!(outcome_size(dec("0.9")), None);
    assert_eq!(coin().limit(Side::Buy, dec("1.5"), dec("1"), Tif::Gtc), Err(OutcomeError::Price(dec("1.5"))));
    assert_eq!(coin().market(Side::Buy, dec("0.5")), Err(OutcomeError::Size(dec("0.5"))));
}

#[test]
fn a_coin_is_spelled_with_a_hash_in_info_and_a_plus_in_balances() {
    let yes = Coin::new(1210, OutcomeSide::Yes);
    assert_eq!(
        (yes.to_string(), yes.balance_name(), yes.encoding()),
        ("#12100".to_owned(), "+12100".to_owned(), 12100)
    );
    assert_eq!(yes.asset().unwrap().id(), 100_012_100);
    assert_eq!(yes.opposite(), Coin::new(1210, OutcomeSide::No));
    assert_eq!(Coin::parse("#12101"), Some(yes.opposite()));
    assert_eq!(Coin::parse("+12100"), Some(yes));
    // A live spot balance row, 2026-10-06: `{"coin":"+92260", ...}`, no `token`.
    assert_eq!(Coin::parse("+92260"), Some(Coin::new(9226, OutcomeSide::Yes)));
    for bad in ["12100", "#", "#12102", "#-1", "@107", "#99999999999"] {
        assert_eq!(Coin::parse(bad), None, "{bad}");
    }
    assert_eq!(Coin::from_asset(yes.asset().unwrap()), Some(yes));
    assert_eq!("#5160".parse::<Coin>(), Ok(coin()));
}
