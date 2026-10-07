//! L1 actions: msgpack, hashed into a connection id, signed as a phantom
//! agent `Agent{source "a", connectionId}` under `Exchange/1/1337/0x0`.

use crate::{
    address::Address,
    body,
    eip712::{Domain, Field, TypedStruct},
    keccak::Keccak256,
    mainnet,
    msgpack::Map,
    signer::Signature,
};

/// `keccak256(msgpack || nonce as u64 big-endian || vault marker)`: `0x00`,
/// or `0x01` and the vault's twenty bytes.
#[must_use]
pub fn connection_id(msgpack: &[u8], nonce: u64, vault: Option<Address>) -> [u8; 32] {
    let sponge = Keccak256::new().chain(msgpack).chain(&nonce.to_be_bytes());
    match vault {
        Some(vault) => sponge.chain(&[0x01]).chain(vault.bytes()),
        None => sponge.chain(&[0x00]),
    }
    .finalize()
}

/// The struct an L1 signature is really over.
#[must_use]
pub fn phantom_agent(connection_id: [u8; 32]) -> TypedStruct<'static> {
    TypedStruct::new("Agent")
        .field("source", Field::String(mainnet::AGENT_SOURCE))
        .field("connectionId", Field::Bytes32(connection_id))
}

/// What the key signs for an L1 action.
#[must_use]
pub fn l1_digest(msgpack: &[u8], nonce: u64, vault: Option<Address>) -> [u8; 32] {
    phantom_agent(connection_id(msgpack, nonce, vault)).digest(&Domain::EXCHANGE)
}

/// An action signed through the phantom agent. A product crate writes
/// [`L1Action::action`], the keys in the order the venue's clients write them,
/// and gets everything else from it.
pub trait L1Action {
    /// The action's keys and values in wire order, which is the hashed order.
    fn action(&self) -> Map;

    #[must_use]
    fn msgpack(&self) -> Vec<u8> {
        self.action().to_msgpack()
    }

    /// The action as posted.
    #[must_use]
    fn wire(&self) -> String {
        self.action().to_json().compact()
    }

    #[must_use]
    fn connection_id(&self, nonce: u64, vault: Option<Address>) -> [u8; 32] {
        connection_id(&self.msgpack(), nonce, vault)
    }

    #[must_use]
    fn typed(&self, nonce: u64, vault: Option<Address>) -> TypedStruct<'static> {
        phantom_agent(self.connection_id(nonce, vault))
    }

    #[must_use]
    fn digest(&self, nonce: u64, vault: Option<Address>) -> [u8; 32] {
        self.typed(nonce, vault).digest(&Domain::EXCHANGE)
    }

    /// For a master wallet signing in the browser. It needs chain 1337.
    #[must_use]
    fn wallet_json(&self, nonce: u64, vault: Option<Address>) -> String {
        self.typed(nonce, vault).wallet_json(&Domain::EXCHANGE)
    }

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
