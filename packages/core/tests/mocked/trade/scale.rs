//! A scale: one action, a leg a level, answered a status a leg.

use df_hyperliquid_core::trade::answer::orders;
use df_hyperliquid_core::trade::orders::scale;
use df_hyperliquid_core::trade::{OrderAnswer, Scale, Side, Tif};

use super::{actions, btc, client, dec};
use crate::support::venue::Venue;

#[tokio::test]
async fn a_scale_rests_a_leg_a_level_and_a_refused_leg_or_batch_is_read_as_refused() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .exchange("scale_one_refused")
            .exchange("order_refused"),
    );
    let market = btc(&client).await;
    let ladder =
        scale(&market, &Scale::new(Side::Buy, dec("0.003"), dec("80000"), dec("79000"), 3, Tif::Alo), None).unwrap();
    let legs = ladder.orders().len();

    let first = client.send(&ladder).await.unwrap();
    assert_eq!(
        orders(&first, legs),
        Ok(vec![
            OrderAnswer::Resting { oid: 77_738_308, cloid: None },
            OrderAnswer::Resting { oid: 77_738_309, cloid: None },
            OrderAnswer::Refused("Insufficient margin to place order.".to_owned()),
        ])
    );
    // One error for the whole batch stands for every leg in it.
    let again = client.send(&ladder).await.unwrap();
    let refused = orders(&again, legs).unwrap();
    assert_eq!(refused, vec![OrderAnswer::Refused("Order must have minimum value of $10.".to_owned()); 3]);
    assert!(refused.iter().all(|leg| !leg.is_placed() && leg.refusal().is_some()));

    let leg = |price: &str| {
        format!(r#"{{"a":0,"b":true,"p":"{price}","s":"0.001","r":false,"t":{{"limit":{{"tif":"Alo"}}}}}}"#)
    };
    let posted =
        format!(r#"{{"type":"order","orders":[{},{},{}],"grouping":"na"}}"#, leg("80000"), leg("79500"), leg("79000"));
    assert_eq!(actions(&client), [posted.clone(), posted]);
    let sent = client.http().actions();
    assert_ne!(sent[0].nonce, sent[1].nonce, "the same action twice is two nonces");
}
