//! A TWAP signed by a wallet outside the client: priced at the mid, posted once, cancelled by the
//! id the venue runs it under.

use df_hyperliquid_core::markets::{Precision, book};
use df_hyperliquid_core::trade::answer::{send_twap, send_twap_cancel};
use df_hyperliquid_core::trade::orders::twap;
use df_hyperliquid_core::trade::{Amount, CancelAnswer, Refusal, Side, TradeError, TwapAnswer, TwapCancel, TwapIntent};

use super::{actions, btc, client, dec, signed};
use crate::support::key;
use crate::support::venue::Venue;

#[tokio::test]
async fn a_twap_signed_by_a_wallet_runs_under_its_id_and_is_cancelled_by_it() {
    let client = client(
        Venue::mainnet()
            .info("meta", "perps/meta_mainnet.json")
            .info("l2Book", "trade/l2_book.json")
            .exchange("twap_running")
            .exchange("twap_cancel_taken"),
    );
    let market = btc(&client).await;
    let mid = book::l2_book(&client, "BTC", Precision::FULL).await.unwrap().mid().unwrap();

    let mut intent = TwapIntent::new(Side::Buy, Amount::Notional(dec("500")), 4).at(mid);
    assert_eq!(twap(&market, &intent), Err(Refusal::Trade(TradeError::TwapMinutes(4))), "refused before signing");
    intent.minutes = 30;
    intent.randomize = true;
    let running = send_twap(&client, signed(&client, &twap(&market, &intent).unwrap())).await.unwrap();
    let TwapAnswer::Running { twap_id } = running else { panic!("{running:?}") };
    assert_eq!(twap_id, 77_738_308);

    let cancelled = send_twap_cancel(&client, signed(&client, &TwapCancel::new(market.asset(), twap_id))).await;
    assert_eq!(cancelled, Ok(CancelAnswer::Cancelled));

    assert_eq!(client.http().read_kinds(), ["meta", "l2Book"]);
    assert_eq!(client.http().reads()[1].compact(), r#"{"type":"l2Book","coin":"BTC"}"#);
    // $500 at the 80858.5 mid is 0.0061836 BTC, cut to five places. A TWAP names no builder.
    assert_eq!(
        actions(&client),
        [
            r#"{"type":"twapOrder","twap":{"a":0,"b":true,"s":"0.00618","r":false,"m":30,"t":true}}"#,
            r#"{"type":"twapCancel","a":0,"t":77738308}"#,
        ]
    );
    assert!(client.http().actions().iter().all(|action| action.signer == Some(key().address())));
}
