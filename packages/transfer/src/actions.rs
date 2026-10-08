//! The user-signed actions that move money: each one's keys in the
//! `@nktkas/hyperliquid` 0.33.2 order, amounts already in normal form,
//! addresses lowercase. Signed by the account's own wallet only.

use hl_core::{Address, Field, Network, NonceKey, UserAction, UserSigned};

use crate::amount::Amount;

/// A balance as `sendAsset` names it: spot.
pub const SPOT_DEX: &str = "spot";
/// The main perp dex's balance, which `sendAsset` names by the empty string.
pub const PERP_DEX: &str = "";

/// `sendAsset`: any token between any two balances of any two accounts. The
/// one transfer a pooled account takes, and the one v1 sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendAsset {
    destination: String,
    source_dex: String,
    destination_dex: String,
    token: String,
    amount: Amount,
    from_sub_account: String,
    nonce: u64,
}

impl SendAsset {
    /// `token` is `NAME:0x<tokenId>`, as `spotMeta` lists it.
    #[must_use]
    pub fn new(
        destination: Address,
        source_dex: &str,
        destination_dex: &str,
        token: &str,
        amount: Amount,
        nonce: u64,
    ) -> SendAsset {
        SendAsset {
            destination: destination.to_hex(),
            source_dex: source_dex.to_owned(),
            destination_dex: destination_dex.to_owned(),
            token: token.to_owned(),
            amount,
            from_sub_account: String::new(),
            nonce,
        }
    }

    /// USDC from one balance to another: to another account from spot (or,
    /// on a standard account, from the perp balance), or to the account itself
    /// to combine its two balances. USDC's token id is the network's own.
    #[must_use]
    pub fn usdc(
        network: Network,
        destination: Address,
        source_dex: &str,
        destination_dex: &str,
        amount: Amount,
        nonce: u64,
    ) -> SendAsset {
        SendAsset::new(destination, source_dex, destination_dex, network.usdc().wire, amount, nonce)
    }

    /// Sent from one of the account's subaccounts rather than the account.
    #[must_use]
    pub fn from_sub_account(mut self, sub_account: Address) -> SendAsset {
        self.from_sub_account = sub_account.to_hex();
        self
    }

    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

impl UserAction for SendAsset {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("sendAsset", "HyperliquidTransaction:SendAsset", NonceKey::Nonce, self.nonce)
            .field("destination", Field::String(&self.destination))
            .field("sourceDex", Field::String(&self.source_dex))
            .field("destinationDex", Field::String(&self.destination_dex))
            .field("token", Field::String(&self.token))
            .field("amount", Field::String(self.amount.as_str()))
            .field("fromSubAccount", Field::String(&self.from_sub_account))
    }
}

/// `spotSend`: a spot token to another account. A pooled account refuses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotSend {
    destination: String,
    token: String,
    amount: Amount,
    time: u64,
}

impl SpotSend {
    #[must_use]
    pub fn new(destination: Address, token: &str, amount: Amount, time: u64) -> SpotSend {
        SpotSend { destination: destination.to_hex(), token: token.to_owned(), amount, time }
    }
}

impl UserAction for SpotSend {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("spotSend", "HyperliquidTransaction:SpotSend", NonceKey::Time, self.time)
            .field("destination", Field::String(&self.destination))
            .field("token", Field::String(&self.token))
            .field("amount", Field::String(self.amount.as_str()))
    }
}

/// `usdSend`: USDC out of the perp balance to another account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsdSend {
    destination: String,
    amount: Amount,
    time: u64,
}

impl UsdSend {
    #[must_use]
    pub fn new(destination: Address, amount: Amount, time: u64) -> UsdSend {
        UsdSend { destination: destination.to_hex(), amount, time }
    }
}

impl UserAction for UsdSend {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("usdSend", "HyperliquidTransaction:UsdSend", NonceKey::Time, self.time)
            .field("destination", Field::String(&self.destination))
            .field("amount", Field::String(self.amount.as_str()))
    }
}

/// `usdClassTransfer`: USDC between the account's own spot and perp balances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsdClassTransfer {
    amount: Amount,
    to_perp: bool,
    nonce: u64,
}

impl UsdClassTransfer {
    #[must_use]
    pub const fn new(amount: Amount, to_perp: bool, nonce: u64) -> UsdClassTransfer {
        UsdClassTransfer { amount, to_perp, nonce }
    }
}

impl UserAction for UsdClassTransfer {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("usdClassTransfer", "HyperliquidTransaction:UsdClassTransfer", NonceKey::Nonce, self.nonce)
            .field("amount", Field::String(self.amount.as_str()))
            .field("toPerp", Field::Bool(self.to_perp))
    }
}

/// `withdraw3`: USDC out of the perp balance to an address on Arbitrum. The
/// venue takes its fee out of the amount (live `withdraw` rows show 1.0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdraw3 {
    destination: String,
    amount: Amount,
    time: u64,
}

impl Withdraw3 {
    #[must_use]
    pub fn new(destination: Address, amount: Amount, time: u64) -> Withdraw3 {
        Withdraw3 { destination: destination.to_hex(), amount, time }
    }

    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

impl UserAction for Withdraw3 {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("withdraw3", "HyperliquidTransaction:Withdraw", NonceKey::Time, self.time)
            .field("destination", Field::String(&self.destination))
            .field("amount", Field::String(self.amount.as_str()))
    }
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
