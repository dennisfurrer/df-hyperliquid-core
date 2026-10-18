//! Outcome order and cancel vectors cross-checked against two independent implementations, and a
//! whole posted body signed with the published key `0x..01`. Mainnet.

use crate::account::{LocalKey, recover};
use crate::client::Network;
use crate::trade::{Cancel, Order, PlaceOrder};
use crate::utils::sign::L1Action;
use crate::utils::{Address, hex};

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
fn the_order_action_hashes_and_signs_to_the_verified_values() {
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
    let key = LocalKey::from_hex("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let published: Address = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf".parse().unwrap();
    assert_eq!(key.address(), published);
    let signature = key.sign_digest(&action.digest(NONCE, None, Network::Mainnet)).unwrap();
    assert_eq!(recover(&action.digest(NONCE, None, Network::Mainnet), &signature), Ok(published));
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

#[test]
fn a_coin_is_spelled_with_a_hash_in_info_and_a_plus_in_balances() {
    let yes = Coin::new(1210, OutcomeSide::Yes);
    assert_eq!(
        (yes.to_string(), yes.balance_name(), yes.encoding()),
        ("#12100".to_owned(), "+12100".to_owned(), 12100)
    );
    assert_eq!((yes.outcome(), yes.side(), yes.asset().unwrap().id()), (1210, OutcomeSide::Yes, 100_012_100));
    assert_eq!(yes.opposite(), Coin::new(1210, OutcomeSide::No));
    assert_eq!(Coin::from_asset(yes.asset().unwrap()), Some(yes));
    // `+92260` is a live spot balance row, 2026-10-06, with no `token`.
    for (text, coin) in [("#12101", yes.opposite()), ("+12100", yes), ("+92260", Coin::new(9226, OutcomeSide::Yes))] {
        assert_eq!(Coin::parse(text), Some(coin), "{text}");
    }
    for bad in ["12100", "#", "#12102", "#-1", "@107", "#99999999999"] {
        assert_eq!(Coin::parse(bad), None, "{bad}");
        assert!(bad.parse::<Coin>().is_err(), "{bad}");
    }
    assert_eq!("#5160".parse::<Coin>(), Ok(coin()));
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

/// A market order goes at the edge of the range as `FrontendMarket`, never `Gtc`; a limit order
/// names a quotable price; either carries whole contracts above zero.
#[test]
fn an_order_on_a_side_is_refused_before_signing_unless_it_can_rest_or_cross() {
    let buy = coin().market(Side::Buy, Qty::new(50)).unwrap();
    let sell = coin().market(Side::Sell, Qty::new(50)).unwrap();
    assert_eq!((buy.price(), buy.tif(), buy.is_market()), (Px::MAX_QUOTE.to_dec(), Some(Tif::FrontendMarket), true));
    assert_eq!((sell.price(), sell.tif(), sell.is_market()), (Px::MIN_QUOTE.to_dec(), Some(Tif::FrontendMarket), true));
    let resting = coin().order(Side::Buy, px("0.35"), Qty::new(50), Tif::Gtc).unwrap();
    assert_eq!(
        PlaceOrder::one(resting).wire(),
        r#"{"type":"order","orders":[{"a":100005160,"b":true,"p":"0.35","s":"50","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#
    );
    let unencodable = Coin::new(u32::MAX, OutcomeSide::Yes);
    let refused = [
        (coin().market(Side::Buy, Qty::ZERO), OutcomeError::Size(Dec::ZERO)),
        (coin().order(Side::Sell, Px::ONE, Qty::new(1), Tif::Gtc), OutcomeError::Price(Dec::ONE)),
        (coin().order(Side::Sell, Px::ZERO, Qty::new(1), Tif::Alo), OutcomeError::Price(Dec::ZERO)),
        (coin().order(Side::Buy, px("0.5"), Qty::ZERO, Tif::Gtc), OutcomeError::Size(Dec::ZERO)),
        (coin().market_within(Side::Buy, px("0.5"), dec("0.51"), Qty::new(1)), OutcomeError::Slippage(dec("0.51"))),
        (coin().market_within(Side::Buy, px("0.5"), dec("-0.01"), Qty::new(1)), OutcomeError::Slippage(dec("-0.01"))),
        (coin().market_within(Side::Buy, px("0.5"), dec("0.01"), Qty::ZERO), OutcomeError::Size(Dec::ZERO)),
        (unencodable.market(Side::Buy, Qty::new(1)), OutcomeError::NoAsset(unencodable)),
        (unencodable.order(Side::Buy, px("0.5"), Qty::new(1), Tif::Gtc), OutcomeError::NoAsset(unencodable)),
        (unencodable.market_within(Side::Buy, px("0.5"), dec("0.01"), Qty::new(1)), OutcomeError::NoAsset(unencodable)),
    ];
    for (order, why) in refused {
        assert_eq!(order, Err(why.clone()), "{why}");
    }
}

/// Two percent over a 0.60 ask to buy and under a 0.58 bid to close; the bound rounds toward its
/// reference and stops at the edge of the range.
#[test]
fn a_market_order_within_a_slippage_is_bounded_toward_its_reference() {
    let cases = [
        (Side::Buy, "0.6", "0.02", "0.612"),
        (Side::Sell, "0.58", "0.02", "0.5684"),
        (Side::Buy, "0.33333", "0.01", "0.33666"),
        (Side::Sell, "0.33333", "0.01", "0.33"),
        (Side::Buy, "0.99", "0.05", "0.99999"),
        (Side::Sell, "0.00001", "0.5", "0.00001"),
        (Side::Buy, "0.58497", "0.02", "0.59666"),
    ];
    let arsenal = Coin::new(1473, OutcomeSide::Yes);
    for (side, reference, slippage, bound) in cases {
        let order = arsenal.market_within(side, px(reference), dec(slippage), Qty::new(16)).unwrap();
        assert_eq!(
            (order.price(), order.size(), order.tif()),
            (dec(bound), dec("16"), Some(Tif::FrontendMarket)),
            "{side} {reference}"
        );
    }
}
