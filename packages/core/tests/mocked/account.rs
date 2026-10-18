//! An account through the [`Client`](df_hyperliquid_core::Client): what it holds and may draw on,
//! its fills, orders and ledger read by window and by id, an agent and a builder fee approved and
//! used, its mode, subaccounts and referral set. The reads answer from mainnet recordings of the
//! accounts named in each module; the owner signing is [`crate::support::key`], an address the venue
//! has never seen.

mod history;
mod holdings;
mod keys;
mod ledger;
mod settings;

use df_hyperliquid_core::utils::Json;
use df_hyperliquid_core::{Address, Dec};

use crate::support::venue::Venue;

/// A unified account, read on 2026-10-08.
const UNIFIED: &str = "0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d";
/// HLP's leader: a standard account with two vault deposits.
const LEADER: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";
/// A builder, standard, with next to nothing in perps on mainnet.
const BUILDER: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// Each `/info` read the venue took, as posted.
fn bodies(venue: &Venue) -> Vec<String> {
    venue.reads().iter().map(Json::compact).collect()
}
