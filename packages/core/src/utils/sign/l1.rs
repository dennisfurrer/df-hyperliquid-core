//! L1 actions: msgpack, hashed into a connection id, signed as a phantom
//! agent `Agent{source, connectionId}` under `Exchange/1/1337/0x0`. The
//! source is `"a"` on mainnet and `"b"` on testnet; the hash has no network.

use crate::account::Signature;
use crate::client::http::body;
use crate::client::network::Network;
use crate::utils::address::Address;
use crate::utils::sign::eip712::{Domain, Field, TypedStruct};
use crate::utils::sign::keccak::Keccak256;
use crate::utils::sign::msgpack::Map;

/// `keccak256(msgpack || nonce as u64 big-endian || vault marker)`: `0x00`,
/// or `0x01` and the vault's twenty bytes.
#[must_use]
pub fn connection_id(msgpack: &[u8], nonce: u64, vault: Option<Address>) -> [u8; 32] {
    connection_id_expiring(msgpack, nonce, vault, None)
}

/// [`connection_id`] for an action posted with `expiresAfter`: the expiry follows the vault
/// marker as `0x00` and the milliseconds as a big-endian u64, as the venue's Python SDK hashes
/// it. With no expiry it is [`connection_id`].
#[must_use]
pub fn connection_id_expiring(
    msgpack: &[u8],
    nonce: u64,
    vault: Option<Address>,
    expires_after: Option<u64>,
) -> [u8; 32] {
    let sponge = Keccak256::new().chain(msgpack).chain(&nonce.to_be_bytes());
    let sponge = match vault {
        Some(vault) => sponge.chain(&[0x01]).chain(vault.bytes()),
        None => sponge.chain(&[0x00]),
    };
    match expires_after {
        Some(expires_after) => sponge.chain(&[0x00]).chain(&expires_after.to_be_bytes()),
        None => sponge,
    }
    .finalize()
}

/// The struct an L1 signature is really over.
#[must_use]
pub fn phantom_agent(connection_id: [u8; 32], network: Network) -> TypedStruct<'static> {
    TypedStruct::new("Agent")
        .field("source", Field::String(network.agent_source()))
        .field("connectionId", Field::Bytes32(connection_id))
}

/// What the key signs for an L1 action on `network`.
#[must_use]
pub fn l1_digest(msgpack: &[u8], nonce: u64, vault: Option<Address>, network: Network) -> [u8; 32] {
    phantom_agent(connection_id(msgpack, nonce, vault), network).digest(&Domain::EXCHANGE)
}

/// An action signed through the phantom agent. An implementor writes
/// [`L1Action::action`], keys in the order the venue's clients write them, and
/// gets everything else from it.
pub trait L1Action {
    /// The action's keys and values in wire order, which is the hashed order.
    fn action(&self) -> Map;

    /// The action as msgpack bytes, the first part of the connection id's hash.
    #[must_use]
    fn msgpack(&self) -> Vec<u8> {
        self.action().to_msgpack()
    }

    /// The action as posted.
    #[must_use]
    fn wire(&self) -> String {
        self.action().to_json().compact()
    }

    /// [`connection_id`] of this action at `nonce`, for `vault` when it acts for one.
    #[must_use]
    fn connection_id(&self, nonce: u64, vault: Option<Address>) -> [u8; 32] {
        connection_id(&self.msgpack(), nonce, vault)
    }

    /// The phantom agent struct the key signs for this action.
    #[must_use]
    fn typed(&self, nonce: u64, vault: Option<Address>, network: Network) -> TypedStruct<'static> {
        phantom_agent(self.connection_id(nonce, vault), network)
    }

    /// What the key signs for this action: the phantom agent's digest under [`Domain::EXCHANGE`].
    #[must_use]
    fn digest(&self, nonce: u64, vault: Option<Address>, network: Network) -> [u8; 32] {
        self.typed(nonce, vault, network).digest(&Domain::EXCHANGE)
    }

    /// For a master wallet signing in the browser. It needs chain 1337.
    #[must_use]
    fn wallet_json(&self, nonce: u64, vault: Option<Address>, network: Network) -> String {
        self.typed(nonce, vault, network).wallet_json(&Domain::EXCHANGE)
    }

    /// The `/exchange` body: `action`, `nonce`, `signature` and `vaultAddress`, `null` when
    /// there is no vault.
    #[must_use]
    fn body(&self, nonce: u64, signature: &Signature, vault: Option<Address>) -> String {
        body::l1(&self.action(), nonce, signature, vault)
    }
}

/// A map is an action already.
impl L1Action for Map {
    fn action(&self) -> Map {
        self.clone()
    }
}

#[cfg(test)]
#[path = "tests/l1.rs"]
mod tests;
