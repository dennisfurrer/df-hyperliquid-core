//! Take profit and stop loss: bracketing an entry, and covering a position already open.

use df_hyperliquid_core::account::clearinghouse_state;
use df_hyperliquid_core::markets::Dex;
use df_hyperliquid_core::trade::answer::orders;
use df_hyperliquid_core::trade::orders::{order, trigger};
use df_hyperliquid_core::trade::{
    Amount, Covers, Exit, Grouping, OrderAnswer, OrderIntent, Side, Tif, Tpsl, TriggerIntent,
};

use super::{actions, btc, client, dec};
use crate::support::key;
use crate::support::venue::Venue;

#[tokio::test]
async fn an_entry_carries_its_exits_grouped_normal_tpsl_and_a_stop_covers_the_open_position() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("clearinghouseState", "perps/clearinghouse_isolated_mainnet.json")
            .exchange("bracket_resting")
            .exchange("trigger_waiting"),
    );
    let market = btc(&client).await;

    let entry = OrderIntent::limit(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), Tif::Gtc)
        .take_profit(Exit::market(dec("84000")))
        .stop_loss(Exit::market(dec("76000")));
    let bracket = order(&market, &entry, None).unwrap();
    assert_eq!(bracket.grouping(), Grouping::NormalTpsl);
    let placed = client.send(&bracket).await.unwrap();
    assert_eq!(
        orders(&placed, bracket.orders().len()),
        Ok(vec![
            OrderAnswer::Resting { oid: 77_738_308, cloid: None },
            OrderAnswer::WaitingForFill,
            OrderAnswer::WaitingForFill
        ])
    );

    let state = clearinghouse_state(&client, key().address(), Dex::Main).await.unwrap();
    let long = state.position("BTC").unwrap();
    let stop =
        TriggerIntent::new(long.closing_side().unwrap(), Covers::Position, Tpsl::StopLoss, Exit::market(dec("78000")));
    let protected = client.send(&trigger(&market, &stop, None).unwrap()).await.unwrap();
    let [answer] = &orders(&protected, 1).unwrap()[..] else { panic!("{protected:?}") };
    assert_eq!(answer, &OrderAnswer::WaitingForTrigger);
    assert!(answer.is_placed() && answer.oid().is_none());

    assert_eq!(client.http().read_kinds(), ["meta", "clearinghouseState"]);
    assert_eq!(
        client.http().reads()[1].compact(),
        format!(r#"{{"type":"clearinghouseState","user":"{}"}}"#, key().address().to_hex())
    );
    // Each exit sells reduce only, at market within 10% of its trigger; the position's stop is size 0.
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"order","orders":[{"a":0,"b":true,"p":"80000","s":"0.00375","r":false,"t":{"limit":{"tif":"Gtc"}}},{"a":0,"b":false,"p":"75600","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"84000","tpsl":"tp"}}},{"a":0,"b":false,"p":"68400","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"76000","tpsl":"sl"}}}],"grouping":"normalTpsl"}"#,
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"70200","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"78000","tpsl":"sl"}}}],"grouping":"positionTpsl"}"#,
        ]
    );
}
