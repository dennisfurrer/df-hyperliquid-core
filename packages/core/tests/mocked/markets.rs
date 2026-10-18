//! Reading the markets through the [`Client`]: a perp on the main dex and on a HIP-3 dex, funding
//! over windows and pages, a spot pair, a market's tape and a HIP-4 outcome. Each journey holds
//! every `/info` body to the exact JSON the venue is asked, and every order to the exact JSON it
//! signed.

mod funding;
mod outcomes;
mod perps;
mod spot;
mod tape;

use df_hyperliquid_core::utils::Json;
use df_hyperliquid_core::{Client, Dec};

use crate::support::venue::Venue;

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

/// Every `/info` body the venue took, in order, as compact JSON.
fn bodies<S>(client: &Client<Venue, S>) -> Vec<String> {
    client.http().reads().iter().map(Json::compact).collect()
}

/// The one order of the one action the venue took, as compact JSON.
fn sent_order<S>(client: &Client<Venue, S>) -> String {
    let actions = client.http().actions();
    let [sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(sent.kind(), "order");
    let orders = sent.action.get("orders").and_then(Json::as_array).unwrap();
    let [order] = orders else { panic!("{orders:?}") };
    order.compact()
}
