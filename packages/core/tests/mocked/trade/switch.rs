//! What stops orders without naming them: the dead man's switch armed and disarmed, and a noop
//! that spends the nonce of an order signed but never sent.

use df_hyperliquid_core::trade::answer::taken;
use df_hyperliquid_core::trade::{Asset, Noop, Order, PlaceOrder, ScheduleCancel, Side, Tif};
use df_hyperliquid_core::utils::Json;

use super::{actions, client, dec, signed};
use crate::support::venue::Venue;
use crate::support::{NOW, clock};

#[tokio::test]
async fn a_dead_man_s_switch_is_armed_and_disarmed_and_a_noop_spends_an_unsent_order_s_nonce() {
    let client = client(Venue::mainnet().exchange("default").exchange("default").exchange("default"));

    let soon = ScheduleCancel::arm(NOW + 4_999, clock()).unwrap_err();
    assert_eq!((soon.time, soon.earliest), (NOW + 4_999, NOW + 5_000));
    let armed = ScheduleCancel::arm(NOW + 60_000, clock()).unwrap();
    assert_eq!(taken(&client.send(&armed).await.unwrap()), Ok(()));
    assert_eq!(taken(&client.send(&ScheduleCancel::disarm()).await.unwrap()), Ok(()));

    let unsent = PlaceOrder::one(Order::limit(Asset::from_id(0), Side::Buy, dec("80000"), dec("0.001"), Tif::Gtc));
    let nonce = Json::parse(&signed(&client, &unsent)).unwrap().get("nonce").and_then(Json::as_u64).unwrap();
    assert_eq!(taken(&client.send_at(&Noop, nonce).await.unwrap()), Ok(()));

    assert_eq!(
        actions(&client),
        [
            format!(r#"{{"type":"scheduleCancel","time":{}}}"#, NOW + 60_000),
            r#"{"type":"scheduleCancel"}"#.to_owned(),
            r#"{"type":"noop"}"#.to_owned(),
        ]
    );
    let sent = client.http().actions();
    assert_eq!(sent[2].nonce, nonce, "the noop takes the order's nonce, so the order cannot land");
}
