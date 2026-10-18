//! What the venue refuses, and what this build cannot read, whichever way the body was signed: by
//! the client, or by a wallet and posted with `trade::answer`. Nothing of it reads as placed.

use df_hyperliquid_core::SendError;
use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::trade::answer::{send_cancels, send_orders, send_taken};
use df_hyperliquid_core::trade::orders::order;
use df_hyperliquid_core::trade::{Amount, Cancel, Modify, OrderAnswer, OrderIntent, OrderRef, Side, Tif};

use super::{actions, btc, client, dec, signed};
use crate::support::venue::Venue;

#[tokio::test]
async fn what_the_venue_refuses_or_this_build_cannot_read_is_never_read_as_placed() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .exchange("refused_whole")
            .exchange("order_unknown")
            .exchange("cancel_taken")
            .exchange("order_refused"),
    );
    let market = btc(&client).await;
    let bid = order(&market, &OrderIntent::limit(Side::Buy, Amount::Size(dec("0.001")), dec("80000"), Tif::Gtc), None)
        .unwrap();

    assert_eq!(
        client.send(&bid).await,
        Err(SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned())))
    );

    let unknown = send_orders(&client, signed(&client, &bid), 1).await.unwrap();
    assert_eq!(unknown, [OrderAnswer::Unknown(r#"{"somethingNew":{"oid":77738308}}"#.to_owned())]);
    assert!(!unknown[0].is_placed() && unknown[0].oid().is_none());

    let two = Cancel::new(vec![(market.asset(), 1), (market.asset(), 2)]);
    let short = send_cancels(&client, signed(&client, &two), 2).await;
    assert!(matches!(short, Err(ApiError::Unreadable { .. })), "one status for two cancels: {short:?}");

    let moved = Modify::new(OrderRef::Oid(77_738_308), bid.orders()[0]);
    let refused = send_taken(&client, signed(&client, &moved)).await;
    assert_eq!(refused, Err(ApiError::Refused("Order must have minimum value of $10.".to_owned())));

    assert_eq!(actions(&client).len(), 4, "each was posted once and never again");
    let kinds: Vec<String> = client.http().actions().iter().map(|sent| sent.kind().to_owned()).collect();
    assert_eq!(kinds, ["order", "order", "cancel", "modify"]);
}
