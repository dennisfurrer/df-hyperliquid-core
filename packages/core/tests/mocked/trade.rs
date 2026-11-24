//! Orders through the [`Client`]: read the market, build the order, sign, send, read the answer.
//! Each action is held whole as the venue took it, and every L1 action must recover to the test
//! key ([`Venue::signed_by`]).

mod builder;
mod outcomes;
mod position;
mod refused;
mod resting;
mod scale;
mod switch;
mod tpsl;
mod trailing;
mod twap;

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::markets::{self, Dex, Precision, perps};
use df_hyperliquid_core::trade::{self, Amount, L1Action, Market, OrderAnswer, OrderIntent, Side};
use df_hyperliquid_core::utils::Json;
use df_hyperliquid_core::{Client, Dec, Network};

use crate::support::venue::Venue;
use crate::support::{clock, key};

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

/// A client on `venue` signing with the test key, which every L1 action must recover to.
fn client(venue: Venue) -> Client<Venue, LocalKey> {
    Client::new(venue.signed_by(key().address()), Network::Mainnet, clock, key())
}

/// BTC on the main dex, as `meta` lists it.
async fn btc(client: &Client<Venue, LocalKey>) -> Market {
    let meta = perps::meta(client, Dex::Main).await.unwrap().unwrap();
    let (asset, btc) = meta.find(0, "BTC").unwrap();
    Market::perp(asset, btc.sz_decimals, btc.max_leverage, btc.allows_cross()).unwrap()
}

/// Every action the venue took, as compact JSON.
fn actions(client: &Client<Venue, LocalKey>) -> Vec<String> {
    client.http().actions().iter().map(|sent| sent.action.compact()).collect()
}

/// The body a wallet outside the client signs, at the client's next nonce.
fn signed(client: &Client<Venue, LocalKey>, action: &impl L1Action) -> String {
    let nonce = client.next_nonce();
    let signature = key().sign_digest(&action.digest(nonce, None, Network::Mainnet)).unwrap();
    action.body(nonce, &signature, None)
}

#[tokio::test]
async fn a_market_buy_reads_btc_signs_frontend_market_and_reads_the_fill() {
    let venue = Venue::mainnet()
        .info("meta", "perps/meta_mainnet.json")
        .info("l2Book", "trade/l2_book.json")
        .exchange("order_filled")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let meta = perps::meta(&client, Dex::Main).await.unwrap().unwrap();
    let (asset, btc) = meta.find(0, "BTC").unwrap();
    let book = markets::book::l2_book(&client, "BTC", Precision::FULL).await.unwrap();
    let market = Market::perp(asset, btc.sz_decimals, btc.max_leverage, btc.allows_cross()).unwrap();
    let intent = OrderIntent::market(Side::Buy, Amount::Notional(dec("20")), book.mid().unwrap(), dec("0.01"));
    let order = trade::orders::order(&market, &intent, None).unwrap();
    let accepted = client.send(&order).await.unwrap();

    assert_eq!(client.http().read_kinds(), ["meta", "l2Book"]);
    let actions = client.http().actions();
    let [sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(sent.kind(), "order");
    let wire = sent.action.get("orders").and_then(Json::as_array).unwrap();
    assert_eq!(wire.len(), 1);
    assert_eq!(wire[0].get("a").and_then(Json::as_u64), Some(0));
    assert_eq!(wire[0].get("b"), Some(&Json::Bool(true)));
    assert_eq!(wire[0].get("t").map(Json::compact).as_deref(), Some(r#"{"limit":{"tif":"FrontendMarket"}}"#));
    // A 1% bound over the 80858.5 mid, cut to five significant figures; $20 of it in BTC's five size decimals.
    assert_eq!(wire[0].get("p").and_then(Json::as_str), Some("81667"));
    assert_eq!(wire[0].get("s").and_then(Json::as_str), Some("0.00024"));

    let answers = trade::answer::orders(&accepted, 1).unwrap();
    assert!(matches!(answers.as_slice(), [OrderAnswer::Filled { .. }]), "{answers:?}");
}
