//! User-signed actions: EIP-712 typed data signed by the account's own wallet
//! under `HyperliquidSignTransaction/1/42161/0x0`. Agents are refused these.
//!
//! One field list drives both the posted action and the typed struct, so the
//! two cannot drift: `type`, `signatureChainId`, `hyperliquidChain`, the
//! action's own keys, then the nonce under its own name, as the
//! `@nktkas/hyperliquid` SDK writes them.

use crate::{
    body,
    eip712::{Domain, Field, TypedStruct},
    mainnet,
    msgpack::Map,
    signer::Signature,
};

/// What an action calls its nonce. The older actions (`usdSend`, `spotSend`,
/// `withdraw3`) say `time`; the number is the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceKey {
    Nonce,
    Time,
}

impl NonceKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            NonceKey::Nonce => "nonce",
            NonceKey::Time => "time",
        }
    }
}

/// A user-signed action, everything that goes into its signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSigned<'a> {
    kind: &'static str,
    primary_type: &'static str,
    fields: Vec<(&'static str, Field<'a>)>,
    nonce_key: NonceKey,
    nonce: u64,
}

impl<'a> UserSigned<'a> {
    /// `kind` is the posted `type` (`withdraw3`), `primary_type` the EIP-712
    /// name (`HyperliquidTransaction:Withdraw`).
    #[must_use]
    pub const fn new(kind: &'static str, primary_type: &'static str, nonce_key: NonceKey, nonce: u64) -> Self {
        UserSigned { kind, primary_type, fields: Vec::new(), nonce_key, nonce }
    }

    /// Appends one of the action's own keys; call order is wire and type order.
    #[must_use]
    pub fn field(mut self, name: &'static str, value: Field<'a>) -> Self {
        self.fields.push((name, value));
        self
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    #[must_use]
    pub const fn nonce(&self) -> u64 {
        self.nonce
    }

    /// The action as posted, in wire order.
    #[must_use]
    pub fn action(&self) -> Map {
        let mut map = Map::new()
            .put("type", self.kind)
            .put("signatureChainId", mainnet::SIGNATURE_CHAIN_ID_HEX)
            .put("hyperliquidChain", mainnet::HYPERLIQUID_CHAIN);
        for (name, value) in &self.fields {
            map = map.put(*name, value.to_wire());
        }
        map.put(self.nonce_key.as_str(), self.nonce)
    }

    /// [`UserSigned::action`] as JSON.
    #[must_use]
    pub fn wire(&self) -> String {
        self.action().to_json().compact()
    }

    /// `hyperliquidChain`, the action's own keys, then the nonce as `uint64`.
    #[must_use]
    pub fn typed(&self) -> TypedStruct<'a> {
        let mut data =
            TypedStruct::new(self.primary_type).field("hyperliquidChain", Field::String(mainnet::HYPERLIQUID_CHAIN));
        for &(name, value) in &self.fields {
            data = data.field(name, value);
        }
        data.field(self.nonce_key.as_str(), Field::Uint64(self.nonce))
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        self.typed().digest(&Domain::USER_SIGNED)
    }

    /// For the wallet. It wants to be on Arbitrum One (42161) to sign this.
    #[must_use]
    pub fn wallet_json(&self) -> String {
        self.typed().wallet_json(&Domain::USER_SIGNED)
    }

    /// The `/exchange` body, with no `vaultAddress`.
    #[must_use]
    pub fn body(&self, signature: &Signature) -> String {
        body::user_signed(&self.action(), self.nonce, signature)
    }
}

/// An action a product crate defines by its [`UserSigned`] form, and gets the
/// posted JSON, typed data, digest, wallet JSON and body from.
pub trait UserAction {
    fn user_signed(&self) -> UserSigned<'_>;

    #[must_use]
    fn nonce(&self) -> u64 {
        self.user_signed().nonce()
    }

    #[must_use]
    fn action(&self) -> Map {
        self.user_signed().action()
    }

    #[must_use]
    fn wire(&self) -> String {
        self.user_signed().wire()
    }

    #[must_use]
    fn typed(&self) -> TypedStruct<'_> {
        self.user_signed().typed()
    }

    #[must_use]
    fn digest(&self) -> [u8; 32] {
        self.user_signed().digest()
    }

    #[must_use]
    fn wallet_json(&self) -> String {
        self.user_signed().wallet_json()
    }

    #[must_use]
    fn body(&self, signature: &Signature) -> String {
        self.user_signed().body(signature)
    }
}
