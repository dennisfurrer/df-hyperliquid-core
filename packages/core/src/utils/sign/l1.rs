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

/// `Domain::EXCHANGE.separator()`: the EIP-712 domain every L1 action is signed under, fixed, so it
/// is not hashed again for each signature. A test holds it to [`Domain::separator`].
pub(crate) const EXCHANGE_SEPARATOR: [u8; 32] = [
    0xd7, 0x92, 0x97, 0xfc, 0xdf, 0x2f, 0xfc, 0xd4, 0xae, 0x22, 0x3d, 0x01, 0xed, 0xaa, 0x2b, 0xa2, 0x14, 0xff, 0x8f,
    0x40, 0x1d, 0x7c, 0x93, 0x00, 0xd9, 0x95, 0xd1, 0x7c, 0x82, 0xaa, 0x40, 0x40,
];
/// `keccak256("Agent(string source,bytes32 connectionId)")`, the phantom agent's type hash.
pub(crate) const AGENT_TYPE_HASH: [u8; 32] = [
    0x26, 0xf0, 0x5c, 0x2f, 0x72, 0x39, 0xb6, 0x98, 0x30, 0x75, 0xe5, 0x83, 0x21, 0x29, 0x2d, 0x77, 0xb3, 0xaa, 0x17,
    0x3d, 0x19, 0xb2, 0x72, 0x57, 0xac, 0x96, 0xab, 0x36, 0x25, 0x70, 0xf5, 0x08,
];
/// `keccak256("a")`, the mainnet source as EIP-712 encodes a string field.
pub(crate) const SOURCE_MAINNET: [u8; 32] = [
    0x3a, 0xc2, 0x25, 0x16, 0x8d, 0xf5, 0x42, 0x12, 0xa2, 0x5c, 0x1c, 0x01, 0xfd, 0x35, 0xbe, 0xbf, 0xea, 0x40, 0x8f,
    0xda, 0xc2, 0xe3, 0x1d, 0xdd, 0x6f, 0x80, 0xa4, 0xbb, 0xf9, 0xa5, 0xf1, 0xcb,
];
/// `keccak256("b")`, the testnet source.
pub(crate) const SOURCE_TESTNET: [u8; 32] = [
    0xb5, 0x55, 0x3d, 0xe3, 0x15, 0xe0, 0xed, 0xf5, 0x04, 0xd9, 0x15, 0x0a, 0xf8, 0x2d, 0xaf, 0xa5, 0xc4, 0x66, 0x7f,
    0xa6, 0x18, 0xed, 0x0a, 0x6f, 0x19, 0xc6, 0x9b, 0x41, 0x16, 0x6c, 0x55, 0x10,
];

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
    agent_digest(connection_id(msgpack, nonce, vault), network)
}

/// The phantom agent's digest under [`Domain::EXCHANGE`] for `connection_id`: the same bytes as
/// `phantom_agent(connection_id, network).digest(&Domain::EXCHANGE)`, from the fixed domain
/// separator, type hash and source hash, so a signature hashes two keccak blocks here instead of
/// eight.
#[must_use]
pub fn agent_digest(connection_id: [u8; 32], network: Network) -> [u8; 32] {
    let source = match network {
        Network::Mainnet => &SOURCE_MAINNET,
        Network::Testnet => &SOURCE_TESTNET,
    };
    let agent = Keccak256::new().chain(&AGENT_TYPE_HASH).chain(source).chain(&connection_id).finalize();
    Keccak256::new().chain(&[0x19, 0x01]).chain(&EXCHANGE_SEPARATOR).chain(&agent).finalize()
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
        agent_digest(self.connection_id(nonce, vault), network)
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
