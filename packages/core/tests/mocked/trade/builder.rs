//! An order through a builder: what the account has approved is read, the approval the order
//! needs is signed first, and the order carries the builder's share of the perp cap.

use df_hyperliquid_core::account::{ApproveBuilderFee, Key, max_builder_fee};
use df_hyperliquid_core::markets::{Precision, book};
use df_hyperliquid_core::trade::answer::{orders, taken};
use df_hyperliquid_core::trade::orders::order;
use df_hyperliquid_core::trade::{
    Amount, Approval, ApprovalRefused, Approved, BuilderFee, BuilderSetting, OrderAnswer, OrderIntent, Side,
};
use df_hyperliquid_core::utils::{Address, Json};

use super::{actions, btc, client, dec};
use crate::support::key;
use crate::support::venue::Venue;

const BUILDER: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";

#[tokio::test]
async fn an_order_through_a_builder_approves_the_fee_first_then_carries_its_share_of_the_cap() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("l2Book", "trade/l2_book.json")
            .info("maxBuilderFee", "trade/max_builder_fee_mainnet.json")
            .exchange("default")
            .exchange("order_filled"),
    );
    let builder: Address = BUILDER.parse().unwrap();
    let setting = BuilderSetting::share_of_cap(builder, 88).unwrap();
    let market = btc(&client).await;
    let mid = book::l2_book(&client, "BTC", Precision::FULL).await.unwrap().mid().unwrap();
    let buy =
        order(&market, &OrderIntent::market(Side::Buy, Amount::Notional(dec("20")), mid, dec("0.01")), Some(&setting))
            .unwrap();

    let approved = Approved::Upto(max_builder_fee(&client, key().address(), builder).await.unwrap());
    assert_eq!(approved, Approved::Upto(BuilderFee::ZERO), "never approved");
    assert!(matches!(setting.approval(&buy, &approved, Key::Agent), Err(ApprovalRefused::AgentCannotApprove { .. })));
    let first = setting.approval(&buy, &approved, Key::Main).unwrap();
    let Approval::ApproveFirst { rate, .. } = first else { panic!("{first:?}") };
    assert_eq!(rate, BuilderFee::from_tenths(880), "the most any market pays, so one approval covers spot too");
    let approve = ApproveBuilderFee::new(builder, rate, client.next_nonce());
    assert_eq!(taken(&client.send_user(&approve).await.unwrap()), Ok(()));
    assert_eq!(setting.approval(&buy, &Approved::Upto(rate), Key::Agent), Ok(Approval::Goes));

    let filled = client.send(&buy).await.unwrap();
    assert!(matches!(orders(&filled, 1).unwrap()[..], [OrderAnswer::Filled { .. }]));

    assert_eq!(client.http().read_kinds(), ["meta", "l2Book", "maxBuilderFee"]);
    assert_eq!(
        client.http().reads()[2].compact(),
        format!(r#"{{"type":"maxBuilderFee","user":"{}","builder":"{BUILDER}"}}"#, key().address().to_hex())
    );
    let sent = client.http().actions();
    let approval = &sent[0];
    assert_eq!(approval.kind(), "approveBuilderFee");
    assert_eq!((approval.chain.as_deref(), approval.signer), (Some("Mainnet"), None), "user-signed, not L1");
    assert_eq!(approval.action.get("maxFeeRate").and_then(Json::as_str), Some("0.88%"));
    assert_eq!(approval.action.get("builder").and_then(Json::as_str), Some(BUILDER));
    assert_eq!(sent[1].signer, Some(key().address()));
    assert_eq!(
        actions(&client)[1],
        format!(
            r#"{{"type":"order","orders":[{{"a":0,"b":true,"p":"81667","s":"0.00024","r":false,"t":{{"limit":{{"tif":"FrontendMarket"}}}}}}],"grouping":"na","builder":{{"b":"{BUILDER}","f":88}}}}"#
        )
    );
}
