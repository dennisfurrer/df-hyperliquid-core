//! HIP-4: a side bought at market through the builder at the outcome cap, quote split into both
//! sides and merged back, and a question's outcomes merged and one negated.

use df_hyperliquid_core::markets::outcomes::{self, Coin, Qty, outcome_meta};
use df_hyperliquid_core::markets::{Precision, book};
use df_hyperliquid_core::trade::answer::{orders, taken};
use df_hyperliquid_core::trade::outcomes::{MergeOutcome, MergeQuestion, NegateOutcome, SplitOutcome};
use df_hyperliquid_core::trade::{BuilderSetting, OrderAnswer, PlaceOrder, Side};
use df_hyperliquid_core::utils::OutcomeSide;

use super::{actions, client, dec};
use crate::support::venue::Venue;

/// Arsenal, a side of the tournament question on mainnet's `outcomeMeta`.
const ARSENAL: u32 = 1473;

#[tokio::test]
async fn an_outcome_side_is_bought_through_the_builder_and_quote_is_split_into_both_sides_and_merged() {
    let client = client(
        Venue::mainnet()
            .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
            .info("l2Book", "hip4/l2_book_14730_mainnet.json")
            .exchange("outcome_filled")
            .exchange("default")
            .exchange("default"),
    );
    let meta = outcome_meta(&client).await.unwrap();
    assert_eq!(meta.outcome(ARSENAL).and_then(|outcome| outcome.quote_token.as_deref()), Some("USDC"));
    let yes = Coin::new(ARSENAL, OutcomeSide::Yes);
    let book =
        outcomes::Book::try_from(book::l2_book(&client, &yes.to_string(), Precision::FULL).await.unwrap()).unwrap();
    let (_, ask) = book.top().unwrap();

    let setting =
        BuilderSetting::share_of_cap("0x6967d7acd1ec5f210e9590666498e0cc5d13a843".parse().unwrap(), 88).unwrap();
    let buy = yes.market_within(Side::Buy, ask, dec("0.02"), Qty::new(20)).unwrap();
    let routed = PlaceOrder::one(buy).through(setting.on_assets([buy.asset()]).unwrap().unwrap());
    let filled = client.send(&routed).await.unwrap();
    let [OrderAnswer::Filled { total_sz, avg_px, .. }] = orders(&filled, 1).unwrap()[..] else { panic!("{filled:?}") };
    assert_eq!((total_sz, avg_px), (dec("92"), dec("0.27225")));

    let split = SplitOutcome::new(ARSENAL, dec("12.50")).unwrap();
    assert_eq!(taken(&client.send(&split).await.unwrap()), Ok(()));
    assert_eq!(taken(&client.send(&MergeOutcome::all(ARSENAL)).await.unwrap()), Ok(()));

    assert_eq!(client.http().read_kinds(), ["outcomeMeta", "l2Book"]);
    assert_eq!(client.http().reads()[1].compact(), r##"{"type":"l2Book","coin":"#14730"}"##);
    // 2% over the 0.58497 ask, cut onto the 0.00001 grid toward it; 880 is 88% of the 1% cap.
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"order","orders":[{"a":100014730,"b":true,"p":"0.59666","s":"20","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":880}}"#,
            r#"{"type":"userOutcome","splitOutcome":{"outcome":1473,"amount":"12.5"}}"#,
            r#"{"type":"userOutcome","mergeOutcome":{"outcome":1473,"amount":null}}"#,
        ]
    );
}

/// The tournament question (51 on mainnet): a full set of its outcomes' Yes merged back, every set
/// the account holds merged, and Arsenal's No turned into a Yes of every other outcome.
#[tokio::test]
async fn a_question_s_outcomes_are_merged_and_one_outcome_negated() {
    let client = client(Venue::mainnet().exchange("default").exchange("default").exchange("default"));
    client.send(&MergeQuestion::new(51, dec("12.50")).unwrap()).await.unwrap();
    client.send(&MergeQuestion::all(51)).await.unwrap();
    client.send(&NegateOutcome::new(51, ARSENAL, dec("3")).unwrap()).await.unwrap();
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"userOutcome","mergeQuestion":{"question":51,"amount":"12.5"}}"#,
            r#"{"type":"userOutcome","mergeQuestion":{"question":51,"amount":null}}"#,
            r#"{"type":"userOutcome","negateOutcome":{"question":51,"outcome":1473,"amount":"3"}}"#,
        ]
    );
}
