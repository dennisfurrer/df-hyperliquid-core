//! An outcome order on each network: the HIP-4 market orders among `l1.json`'s cases on mainnet, and one naming
//! testnet's own outcome from its recorded `outcomeMeta` (`fixtures/hip4`), signed for testnet.

use df_hyperliquid_core::account::{LocalKey, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::markets::outcomes::px::Qty;
use df_hyperliquid_core::markets::outcomes::{Coin, Fields, OutcomeMeta};
use df_hyperliquid_core::trade::{PlaceOrder, Side};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Json, OutcomeSide, hex};

use super::{AGENT_KEY, list, number, parse, text};

const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));
const TESTNET_META: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_meta_testnet.json"));

#[test]
fn outcome_market_orders_are_frontend_market_at_the_edges() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let cases = list(&file, "cases");
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let orders = [
        ("order_hip4_frontend_market_buy", Coin::new(1234, OutcomeSide::Yes).market(Side::Buy, Qty::new(25))),
        ("order_hip4_frontend_market_sell", Coin::new(1234, OutcomeSide::No).market(Side::Sell, Qty::new(10))),
    ];
    for (name, order) in orders {
        let case = case(name);
        assert_eq!(case.get("isTestnet"), Some(&Json::Bool(false)), "{name} was made for mainnet");
        let action = PlaceOrder::one(order.unwrap());
        let nonce = number(case, "nonce");
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}");
        assert_eq!(hex::encode_prefixed(&action.msgpack()), text(case, "msgpack"), "{name}: the msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        let signature = key.sign_digest(&action.digest(nonce, None, Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), text(expected, "r"), "{name}: r");
        assert_eq!(signature.s_hex(), text(expected, "s"), "{name}: s");
        assert_eq!(u64::from(signature.v()), number(expected, "v"), "{name}: v");
    }
}

/// On testnet the outcome ids are testnet's own: the order below names
/// testnet's daily BTC market by the id its recorded `outcomeMeta` gives,
/// and is signed for testnet. An L1 hash names no network; the digest does,
/// so the same action signed for mainnet is a different signature.
#[test]
fn a_testnet_order_names_testnet_s_own_outcome_and_signs_for_testnet() {
    let meta: OutcomeMeta = serde_json::from_str(TESTNET_META).unwrap();
    let daily = meta
        .outcomes
        .iter()
        .find(|row| {
            let fields = Fields::parse(&row.description);
            let field = |key: &str| fields.text(key);
            field("underlying").as_deref() == Some("BTC")
                && field("targetPrice").as_deref() == Some("83136")
                && field("expiry").as_deref() == Some("20261009-0300")
        })
        .expect("testnet's daily BTC market");
    let order = daily.coin(OutcomeSide::Yes).market(Side::Buy, Qty::new(25)).unwrap();
    assert_eq!(order.asset().id(), 100_231_360, "from testnet's data, not a mainnet constant");
    let action = PlaceOrder::one(order);
    let key = LocalKey::from_hex(AGENT_KEY).unwrap();
    let nonce = 1_791_417_600_000;
    let testnet = action.digest(nonce, None, Network::Testnet);
    let signature = key.sign_digest(&testnet).unwrap();
    assert_eq!(recover(&testnet, &signature), Ok(key.address()));
    let mainnet = action.digest(nonce, None, Network::Mainnet);
    assert_ne!(testnet, mainnet);
    assert_ne!(recover(&mainnet, &signature), Ok(key.address()), "a testnet signature is no mainnet one");
}
