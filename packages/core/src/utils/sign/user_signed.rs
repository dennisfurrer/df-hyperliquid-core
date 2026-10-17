//! User-signed actions: EIP-712 typed data signed by the account's own wallet
//! (never an agent) under `HyperliquidSignTransaction/1/<chain>/0x0`, the chain
//! being the network's signature chain (42161 on mainnet, 421614 on testnet).
//!
//! One field list drives both the posted action and the typed struct: `type`,
//! `signatureChainId`, `hyperliquidChain`, the action's own keys, then the
//! nonce under its own name, as the `@nktkas/hyperliquid` SDK writes them.

use crate::account::Signature;
use crate::client::http::body;
use crate::client::network::Network;
use crate::utils::sign::eip712::{Domain, Field, TypedStruct};
use crate::utils::sign::msgpack::Map;

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

    /// The action as posted on `network`, in wire order.
    #[must_use]
    pub fn action(&self, network: Network) -> Map {
        let mut map = Map::new()
            .put("type", self.kind)
            .put("signatureChainId", network.signature_chain_id_hex())
            .put("hyperliquidChain", network.hyperliquid_chain());
        for (name, value) in &self.fields {
            map = map.put(*name, value.to_wire());
        }
        map.put(self.nonce_key.as_str(), self.nonce)
    }

    /// [`UserSigned::action`] as JSON.
    #[must_use]
    pub fn wire(&self, network: Network) -> String {
        self.action(network).to_json().compact()
    }

    /// `hyperliquidChain`, the action's own keys, then the nonce as `uint64`.
    #[must_use]
    pub fn typed(&self, network: Network) -> TypedStruct<'a> {
        let mut data =
            TypedStruct::new(self.primary_type).field("hyperliquidChain", Field::String(network.hyperliquid_chain()));
        for &(name, value) in &self.fields {
            data = data.field(name, value);
        }
        data.field(self.nonce_key.as_str(), Field::Uint64(self.nonce))
    }

    #[must_use]
    pub fn digest(&self, network: Network) -> [u8; 32] {
        self.typed(network).digest(&Domain::user_signed(network))
    }

    /// For the wallet. It wants to be on the network's signature chain to
    /// sign this: Arbitrum One, or Arbitrum Sepolia on testnet.
    #[must_use]
    pub fn wallet_json(&self, network: Network) -> String {
        self.typed(network).wallet_json(&Domain::user_signed(network))
    }

    /// The `/exchange` body, with no `vaultAddress`.
    #[must_use]
    pub fn body(&self, signature: &Signature, network: Network) -> String {
        body::user_signed(&self.action(network), self.nonce, signature)
    }
}

/// An action defined by its [`UserSigned`] form, from which come the posted
/// JSON, typed data, digest, wallet JSON and body.
pub trait UserAction {
    fn user_signed(&self) -> UserSigned<'_>;

    #[must_use]
    fn nonce(&self) -> u64 {
        self.user_signed().nonce()
    }

    #[must_use]
    fn action(&self, network: Network) -> Map {
        self.user_signed().action(network)
    }

    #[must_use]
    fn wire(&self, network: Network) -> String {
        self.user_signed().wire(network)
    }

    #[must_use]
    fn typed(&self, network: Network) -> TypedStruct<'_> {
        self.user_signed().typed(network)
    }

    #[must_use]
    fn digest(&self, network: Network) -> [u8; 32] {
        self.user_signed().digest(network)
    }

    #[must_use]
    fn wallet_json(&self, network: Network) -> String {
        self.user_signed().wallet_json(network)
    }

    #[must_use]
    fn body(&self, signature: &Signature, network: Network) -> String {
        self.user_signed().body(signature, network)
    }
}
