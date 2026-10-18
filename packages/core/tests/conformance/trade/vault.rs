//! `vault.json`, by the venue's Python SDK 0.19.0 acting for a subaccount.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::trade::{Cancel, Side, Tpsl, orders};
use df_hyperliquid_core::utils::Json;
use df_hyperliquid_core::utils::sign::L1Action;

use super::{btc, check, exit, id, market_buy, parse, text};

const VAULT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/vault.json"));

/// Python SDK 0.19.0 on a subaccount: an entry, its stop and a cancel, each
/// hashed with the subaccount as the vault and signed by its master's agent.
#[test]
fn the_python_sdk_s_subaccount_orders_sign_the_same() {
    let file = parse(VAULT);
    let mut checked = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let key = LocalKey::from_hex(text(case, "key")).unwrap();
        assert_eq!(key.address().to_hex(), text(case, "address").to_lowercase(), "{name}");
        let typed: Box<dyn L1Action> = match name {
            // a dummy action, not an order
            "sdk_test_dummy" => continue,
            "order_market_entry" => {
                Box::new(orders::order(&btc(), &market_buy("0.00118", "84100.5", "0.005", 1), None).unwrap())
            }
            "order_stop_market" => Box::new(
                orders::trigger(&btc(), &exit(Side::Sell, "0.00118", Tpsl::StopLoss, "77372", Some(2)), None).unwrap(),
            ),
            "cancel_stop" => Box::new(Cancel::one(id(0), 1002)),
            other => panic!("no typed builder for {other}"),
        };
        check(name, typed.as_ref(), case, &key);
        checked += 1;
    }
    assert_eq!(checked, 3);
}
