//! A position already open: closed at market, its leverage changed within the tier it reaches, and
//! margin added to it.

use df_hyperliquid_core::account::clearinghouse_state;
use df_hyperliquid_core::markets::{Dex, perps};
use df_hyperliquid_core::trade::answer::{orders, taken};
use df_hyperliquid_core::trade::margin::{margin, update_leverage_for};
use df_hyperliquid_core::trade::orders::close;
use df_hyperliquid_core::trade::{Close, LeverageError, MarginMode, Market, OrderAnswer};

use super::{actions, btc, client, dec};
use crate::support::key;
use crate::support::venue::Venue;

#[tokio::test]
async fn closing_the_open_long_sells_all_of_it_reduce_only_at_market() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("clearinghouseState", "perps/clearinghouse_isolated_mainnet.json")
            .exchange("order_filled"),
    );
    let market = btc(&client).await;
    let state = clearinghouse_state(&client, key().address(), Dex::Main).await.unwrap();
    let long = state.position("BTC").unwrap();
    let closing = close(&market, &Close::new(long.szi, long.mark().unwrap(), dec("0.01")), None).unwrap();
    let filled = client.send(&closing).await.unwrap();
    assert!(matches!(orders(&filled, 1).unwrap()[..], [OrderAnswer::Filled { oid: 77_747_314, .. }]));

    // 5101.4205 of position over 0.06075 marks it at 83974; a sell is bounded 1% under, toward the mark.
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"83135","s":"0.06075","r":true,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
        ]
    );
}

#[tokio::test]
async fn leverage_is_set_within_the_tier_the_position_reaches_then_isolated_margin_is_added() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("clearinghouseState", "perps/clearinghouse_isolated_mainnet.json")
            .exchange("default")
            .exchange("default"),
    );
    let meta = perps::meta(&client, Dex::Main).await.unwrap().unwrap();
    let (asset, perp) = meta.find(0, "BTC").unwrap();
    let table = meta.table_of(perp).unwrap();
    let market = Market::perp(asset, perp.sz_decimals, perp.max_leverage, perp.allows_cross()).unwrap();
    let state = clearinghouse_state(&client, key().address(), Dex::Main).await.unwrap();
    let long = state.position("BTC").unwrap();
    let mode = long.leverage.mode();
    assert_eq!((mode, long.leverage.value()), (MarginMode::Isolated, 4));

    let refused = update_leverage_for(asset, perp, &table, MarginMode::Cross, 41, long.position_value);
    assert_eq!(refused, Err(LeverageError::TooHigh { name: "BTC".to_owned(), max: 40, asked: 41 }));
    let set = update_leverage_for(asset, perp, &table, mode, 10, long.position_value).unwrap();
    assert_eq!(taken(&client.send(&set).await.unwrap()), Ok(()));
    let added = margin(&market, long.szi, mode, dec("12.5")).unwrap();
    assert_eq!(taken(&client.send(&added).await.unwrap()), Ok(()));

    assert_eq!(client.http().read_kinds(), ["meta", "clearinghouseState"]);
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"updateLeverage","asset":0,"isCross":false,"leverage":10}"#,
            r#"{"type":"updateIsolatedMargin","asset":0,"isBuy":true,"ntli":12500000}"#,
        ]
    );
}
