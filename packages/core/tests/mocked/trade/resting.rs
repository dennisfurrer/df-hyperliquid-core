//! An order on the book: placed with a client id, moved, cancelled by either id, and a listed
//! stop moved in place.

use df_hyperliquid_core::account::history::frontend_open_orders;
use df_hyperliquid_core::markets::{Dex, perps};
use df_hyperliquid_core::trade::answer::{cancels, orders, taken};
use df_hyperliquid_core::trade::manage::{self, Change};
use df_hyperliquid_core::trade::orders::{TRIGGER_SLIPPAGE, order};
use df_hyperliquid_core::trade::{
    Amount, Cancel, CancelAnswer, CancelByCloid, Cloid, Covers, ExitPrice, Market, Modify, OrderAnswer, OrderIntent,
    OrderRef, Side, Tif,
};

use super::{actions, btc, client, dec};
use crate::support::venue::Venue;
use crate::support::{NOW, key};

const CLOID: &str = "0x0000000000000000000000000000beef";

#[tokio::test]
async fn a_limit_order_rests_is_moved_by_its_cloid_and_cancelled_by_oid_then_by_cloid() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .exchange("order_resting")
            .exchange("default")
            .exchange("cancel_taken")
            .exchange("cancel_refused"),
    );
    let market = btc(&client).await;
    let cloid: Cloid = CLOID.parse().unwrap();

    let bid = OrderIntent::limit(Side::Buy, Amount::Size(dec("0.001")), dec("80000.7"), Tif::Gtc).with_cloid(cloid);
    let placed = client.send(&order(&market, &bid, None).unwrap()).await.unwrap();
    let [OrderAnswer::Resting { oid, .. }] = orders(&placed, 1).unwrap()[..] else { panic!("{placed:?}") };
    assert_eq!(oid, 77_738_308);

    let higher = OrderIntent::limit(Side::Buy, Amount::Size(dec("0.002")), dec("80500"), Tif::Gtc).with_cloid(cloid);
    let moved = order(&market, &higher, None).unwrap().orders()[0];
    assert_eq!(taken(&client.send(&Modify::new(OrderRef::Cloid(cloid), moved)).await.unwrap()), Ok(()));

    let by_oid = client.send(&Cancel::one(market.asset(), oid)).await.unwrap();
    assert_eq!(cancels(&by_oid, 1), Ok(vec![CancelAnswer::Cancelled]));
    let by_cloid = client.send(&CancelByCloid::one(market.asset(), cloid)).await.unwrap();
    assert_eq!(
        cancels(&by_cloid, 1),
        Ok(vec![CancelAnswer::Refused("Order was never placed, already canceled, or filled.".to_owned())])
    );

    let resting = format!(r#""r":false,"t":{{"limit":{{"tif":"Gtc"}}}},"c":"{CLOID}""#);
    assert_eq!(
        actions(&client),
        [
            format!(
                r#"{{"type":"order","orders":[{{"a":0,"b":true,"p":"80000","s":"0.001",{resting}}}],"grouping":"na"}}"#
            ),
            format!(
                r#"{{"type":"modify","oid":"{CLOID}","order":{{"a":0,"b":true,"p":"80500","s":"0.002",{resting}}}}}"#
            ),
            r#"{"type":"cancel","cancels":[{"a":0,"o":77738308}]}"#.to_owned(),
            format!(r#"{{"type":"cancelByCloid","cancels":[{{"asset":0,"cloid":"{CLOID}"}}]}}"#),
        ]
    );
    let sent = client.http().actions();
    assert!(sent.iter().all(|action| action.signer == Some(key().address()) && action.vault.is_none()));
    assert_eq!(sent.iter().map(|action| action.nonce).collect::<Vec<_>>(), [NOW, NOW + 1, NOW + 2, NOW + 3]);
}

#[tokio::test]
async fn a_position_stop_the_venue_lists_is_moved_in_place_and_always_places() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("frontendOpenOrders", "trade/frontend_open_orders.json")
            .exchange("trigger_waiting"),
    );
    let meta = perps::meta(&client, Dex::Main).await.unwrap().unwrap();
    let (asset, ens) = meta.find(0, "ENS").unwrap();
    let market = Market::perp(asset, ens.sz_decimals, ens.max_leverage, ens.allows_cross()).unwrap();
    let listed = frontend_open_orders(&client, key().address(), Dex::Main).await.unwrap();
    let stop = listed.iter().find(|order| order.coin == "ENS").unwrap();
    assert!(stop.is_position_tpsl && stop.sz.is_zero(), "{stop:?}");

    let lower = Change::Trigger {
        trigger: dec("5.9"),
        price: ExitPrice::Market { slippage: TRIGGER_SLIPPAGE },
        size: Covers::Position,
    };
    let modify = manage::modify(&market, stop, &lower).unwrap();
    let accepted = client.send(&modify).await.unwrap();
    assert_eq!(orders(&accepted, modify.modifies().len()), Ok(vec![OrderAnswer::WaitingForTrigger]));

    assert_eq!(client.http().read_kinds(), ["meta", "frontendOpenOrders"]);
    assert_eq!(
        client.http().reads()[1].compact(),
        format!(r#"{{"type":"frontendOpenOrders","user":"{}"}}"#, key().address().to_hex())
    );
    // The short's stop buys, bounded 10% over the new trigger; size 0 still covers the whole position,
    // and the stop keeps the client order id it was listed with.
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"batchModify","modifies":[{"oid":569021780586,"order":{"a":101,"b":true,"p":"6.49","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"5.9","tpsl":"sl"}},"c":"0xba5ed11067f2cc01ba5ed10000ba5ed1"}}],"a":true}"#
        ]
    );
}
