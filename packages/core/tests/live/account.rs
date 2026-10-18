//! Every account read, from mainnet, at the addresses the recordings were taken with: what they
//! hold, what they did, who signs for them and how they are set up. Each answer must parse and
//! have its recording's shape.

mod history;
mod holdings;
mod keys;
mod settings;

use df_hyperliquid_core::Address;

/// A unified account, and its subaccount.
const UNIFIED: &str = "0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d";
const UNIFIED_SUB: &str = "0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f";
/// HLP's leader, a standard account.
const LEADER: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";
/// A builder.
const BUILDER: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";
/// An address the venue has never seen: the mocked suite's owner.
const FRESH: &str = "0x13765b62653adb4e912c9b1cbdc789efa883c106";

fn address(text: &str) -> Address {
    text.parse().unwrap()
}
