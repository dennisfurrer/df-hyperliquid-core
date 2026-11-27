//! Everything an L1 signature is over, laid out in the order it is built.
//!
//! "User or API Wallet 0x... does not exist" is the venue saying the address it recovered from a
//! signature is not one it knows, which almost always means what was signed is not what it hashed.
//! Each stage here is deterministic, so each can be shown, compared with another SDK's, or timed:
//! the msgpack bytes, the action hash over them with the nonce, the vault and an optional expiry,
//! the EIP-712 payload around that hash, the digest a key signs, and the address a signature
//! recovers to. Every stage uses the encoders [`L1Action::digest`] and [`L1Action::body`] use.

#[cfg(feature = "sign")]
use crate::account::{SignError, Signature};
use crate::client::network::Network;
use crate::utils::address::Address;
use crate::utils::sign::eip712::Domain;
use crate::utils::sign::l1::{L1Action, connection_id_expiring, phantom_agent};

/// The stages of one L1 signature, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakdown {
    /// The action as msgpack, keys in wire order: the first part of the hash.
    pub msgpack: Vec<u8>,
    /// `keccak256(msgpack || nonce || vault marker || optional expiry)`: the connection id the
    /// phantom agent carries.
    pub action_hash: [u8; 32],
    /// The EIP-712 typed data a wallet is asked to sign (`eth_signTypedData_v4`): the phantom
    /// agent `Agent{source, connectionId}` under `Exchange/1/1337/0x0`, as JSON.
    pub payload: String,
    /// What a key signs: the EIP-712 digest of the payload.
    pub digest: [u8; 32],
}

impl Breakdown {
    /// The stages for `action` at `nonce`, for `vault` when it acts for one, on `network`.
    #[must_use]
    pub fn of<A: L1Action + ?Sized>(action: &A, nonce: u64, vault: Option<Address>, network: Network) -> Breakdown {
        Breakdown::expiring(action, nonce, vault, None, network)
    }

    /// [`Breakdown::of`] for an action posted with `expiresAfter` (milliseconds); `None` is
    /// [`Breakdown::of`].
    #[must_use]
    pub fn expiring<A: L1Action + ?Sized>(
        action: &A,
        nonce: u64,
        vault: Option<Address>,
        expires_after: Option<u64>,
        network: Network,
    ) -> Breakdown {
        let msgpack = action.msgpack();
        let action_hash = connection_id_expiring(&msgpack, nonce, vault, expires_after);
        let agent = phantom_agent(action_hash, network);
        Breakdown {
            payload: agent.wallet_json(&Domain::EXCHANGE),
            digest: agent.digest(&Domain::EXCHANGE),
            msgpack,
            action_hash,
        }
    }

    /// The last stage: the address `signature` recovers to over this digest. The venue answers
    /// "does not exist" for any address but the signer's own.
    #[cfg(feature = "sign")]
    pub fn recover(&self, signature: &Signature) -> Result<Address, SignError> {
        crate::account::recover(&self.digest, signature)
    }
}

#[cfg(test)]
#[path = "tests/breakdown.rs"]
mod tests;
