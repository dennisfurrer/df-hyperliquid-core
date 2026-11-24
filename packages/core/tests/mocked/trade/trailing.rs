//! A trailing stop on BTC, sent as its own action and answered with the stop's order id, as the
//! exchange endpoint docs answer one.

use df_hyperliquid_core::trade::{Retracement, Side, TrailingStop};

use super::{actions, btc, client, dec};
use crate::support::venue::Venue;

/// The docs' answer to a trailing stop.
const PLACED: &str = r#"{"status":"ok","response":{"type":"trailingStop","data":{"oid":77738308}}}"#;

#[tokio::test]
async fn a_trailing_stop_is_signed_by_the_account_and_answered_with_its_order_id() {
    let client = client(Venue::mainnet().info("meta", "perps/meta_mainnet.json").exchange_text(PLACED));
    let market = btc(&client).await;
    let size = market.grid().size(dec("0.0123")).unwrap();
    let stop = TrailingStop::new(market.asset(), Side::Sell, size, Retracement::Percent(dec("1.5")))
        .and_then(|stop| stop.activated_at(dec("90000")))
        .unwrap()
        .reduce_only();

    let accepted = client.send(&stop).await.unwrap();

    assert_eq!(
        actions(&client),
        [
            r#"{"type":"trailingStop","asset":0,"isBuy":false,"sz":"0.0123","reduceOnly":true,"retracement":{"pct":"1.5%"},"activationPx":"90000"}"#
        ]
    );
    assert_eq!(accepted.kind, "trailingStop");
    assert_eq!(accepted.data.and_then(|data| data.get("oid").and_then(serde_json::Value::as_u64)), Some(77_738_308));
}
