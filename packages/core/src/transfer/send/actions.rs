//! The actions that move money, keys in the `@nktkas/hyperliquid` 0.33.2 order, amounts in normal
//! form, addresses lowercase. The user-signed ones are signed by the account's own wallet only;
//! [`AgentSendAsset`], an L1 action, by an agent for its master.

use crate::account::{ActionKind, Keeping, Signers};
use crate::client::Network;
use crate::transfer::send::amount::Amount;
use crate::utils::Address;
use crate::utils::sign::{Field, L1Action, Map, NonceKey, UserAction, UserSigned};

/// A balance as `sendAsset` names it: spot.
pub const SPOT_DEX: &str = "spot";
/// The main perp dex's balance, which `sendAsset` names by the empty string.
pub const PERP_DEX: &str = "";

/// `sendAsset`: any token between any two balances of any two accounts, and the one transfer a
/// pooled account takes.
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
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::SendAsset.signers();

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
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::SpotSend.signers();

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
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::UsdSend.signers();

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
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::UsdClassTransfer.signers();

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

/// `withdraw3`: USDC out of the perp balance to an address on Arbitrum. The venue takes its fee
/// (1 USDC in live `withdraw` rows) out of the amount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdraw3 {
    destination: String,
    amount: Amount,
    time: u64,
}

impl Withdraw3 {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::Withdraw3.signers();

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

/// A spot token out of the account: `spotSend` for a separate account, and `sendAsset` from spot
/// to spot for a pooled one (which refuses `spotSend`) or one whose mode is not known (every kind
/// takes it). Either goes to a person, or through a system address to HyperEVM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotOut {
    SpotSend(SpotSend),
    SendAsset(SendAsset),
}

impl SpotOut {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = SpotSend::KEY;

    /// `token` is `NAME:0x<tokenId>`.
    #[must_use]
    pub fn new(destination: Address, token: &str, amount: Amount, keeping: Keeping, nonce: u64) -> SpotOut {
        match keeping {
            Keeping::Apart | Keeping::Native => SpotOut::SpotSend(SpotSend::new(destination, token, amount, nonce)),
            Keeping::Unified | Keeping::Portfolio | Keeping::Unknown => {
                SpotOut::SendAsset(SendAsset::new(destination, SPOT_DEX, SPOT_DEX, token, amount, nonce))
            }
        }
    }
}

impl UserAction for SpotOut {
    fn user_signed(&self) -> UserSigned<'_> {
        match self {
            SpotOut::SpotSend(send) => send.user_signed(),
            SpotOut::SendAsset(send) => send.user_signed(),
        }
    }
}

/// `agentSendAsset`: `sendAsset`'s fields as an L1 action, which an agent signs for its master.
/// The destination is the account itself, so it only moves between the account's own balances.
/// The action carries its nonce and is signed with that same nonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSendAsset {
    account: Address,
    source_dex: String,
    destination_dex: String,
    token: String,
    amount: Amount,
    nonce: u64,
}

impl AgentSendAsset {
    /// L1: an agent the account approved, or the account's own key.
    pub const KEY: Signers = ActionKind::AgentSendAsset.signers();

    /// From `source_dex` to `destination_dex` of `account`, which is also the
    /// destination. `token` is `NAME:0x<tokenId>`.
    #[must_use]
    pub fn new(
        account: Address,
        source_dex: &str,
        destination_dex: &str,
        token: &str,
        amount: Amount,
        nonce: u64,
    ) -> AgentSendAsset {
        AgentSendAsset {
            account,
            source_dex: source_dex.to_owned(),
            destination_dex: destination_dex.to_owned(),
            token: token.to_owned(),
            amount,
            nonce,
        }
    }

    /// The nonce inside the action, which is also the one it is signed with.
    #[must_use]
    pub const fn nonce(&self) -> u64 {
        self.nonce
    }

    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.amount
    }
}

impl L1Action for AgentSendAsset {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "agentSendAsset")
            .put("destination", self.account.to_hex())
            .put("sourceDex", self.source_dex.clone())
            .put("destinationDex", self.destination_dex.clone())
            .put("token", self.token.clone())
            .put("amount", self.amount.as_str().to_owned())
            .put("fromSubAccount", "")
            .put("nonce", self.nonce)
    }
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
