//! Multi-sig: an L1 action sent for an account its signers hold together. Each signer signs the
//! action nested as `[multiSigUser, outerSigner, action]`, an L1 action like any other; the outer
//! signer, the one of them that posts it, wraps the action and every signature as `multiSig` and
//! signs that as `SendMultiSig`, a user-signed action over its hash. Both as the venue's Python SDK
//! nests and signs them.

use crate::account::Signature;
use crate::client::http::body;
use crate::client::network::Network;
use crate::utils::address::Address;
use crate::utils::sign::eip712::{Domain, Field, TypedStruct};
use crate::utils::sign::l1::{L1Action, connection_id, phantom_agent};
use crate::utils::sign::msgpack::{Map, Value};
use crate::utils::sign::user_signed::{NonceKey, UserSigned};

/// An L1 action for the multi-sig account `user`, posted by `outer_signer`, one of its signers,
/// with each signature collected for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSig {
    /// `multiSigUser`: the multi-sig account the action is for.
    pub user: Address,
    /// `outerSigner`: the signer that posts the action and signs the outer `SendMultiSig`.
    pub outer_signer: Address,
    /// The action as the account would send it alone, `type` and all.
    pub action: Map,
    /// Each signer's signature over [`MultiSig::payload_digest`], in the order collected.
    pub signatures: Vec<Signature>,
}

impl MultiSig {
    /// No signatures yet.
    #[must_use]
    pub fn new(user: Address, outer_signer: Address, action: &impl L1Action) -> Self {
        MultiSig { user, outer_signer, action: action.action(), signatures: Vec::new() }
    }

    /// With the signatures collected, in the order they are posted.
    #[must_use]
    pub fn signed(mut self, signatures: Vec<Signature>) -> Self {
        self.signatures = signatures;
        self
    }

    /// What each signer signs: `[multiSigUser, outerSigner, action]`, the addresses lower case.
    #[must_use]
    pub fn payload(&self) -> Value {
        Value::Array(vec![
            Value::from(self.user.to_hex()),
            Value::from(self.outer_signer.to_hex()),
            Value::Map(self.action.clone()),
        ])
    }

    /// The payload's connection id, hashed as an L1 action's is.
    #[must_use]
    pub fn payload_connection_id(&self, nonce: u64, vault: Option<Address>) -> [u8; 32] {
        connection_id(&self.payload().to_msgpack(), nonce, vault)
    }

    /// What each signer's key signs on `network`: the payload's phantom agent.
    #[must_use]
    pub fn payload_digest(&self, nonce: u64, vault: Option<Address>, network: Network) -> [u8; 32] {
        phantom_agent(self.payload_connection_id(nonce, vault), network).digest(&Domain::EXCHANGE)
    }

    /// `{"type":"multiSig","signatureChainId","signatures","payload":{"multiSigUser","outerSigner","action"}}`,
    /// as posted on `network`, each signature `{r,s,v}`.
    #[must_use]
    pub fn action(&self, network: Network) -> Map {
        self.untyped(network)
            .into_pairs()
            .fold(Map::new().put("type", "multiSig"), |map, (key, value)| map.put(key, value))
    }

    /// [`MultiSig::action`] without its `type`, which is what is hashed.
    fn untyped(&self, network: Network) -> Map {
        let signatures = self.signatures.iter().map(|signature| {
            Value::Map(
                Map::new().put("r", signature.r_hex()).put("s", signature.s_hex()).put("v", u64::from(signature.v())),
            )
        });
        Map::new()
            .put("signatureChainId", network.signature_chain_id_hex())
            .put("signatures", Value::Array(signatures.collect()))
            .put(
                "payload",
                Map::new()
                    .put("multiSigUser", self.user.to_hex())
                    .put("outerSigner", self.outer_signer.to_hex())
                    .put("action", self.action.clone()),
            )
    }

    /// `multiSigActionHash`: [`MultiSig::action`] without its `type`, hashed as an L1 action's is.
    #[must_use]
    pub fn action_hash(&self, nonce: u64, vault: Option<Address>, network: Network) -> [u8; 32] {
        connection_id(&self.untyped(network).to_msgpack(), nonce, vault)
    }

    /// `SendMultiSig`, the hash and the nonce, user-signed on `network`.
    fn send(&self, nonce: u64, vault: Option<Address>, network: Network) -> UserSigned<'static> {
        UserSigned::new("multiSig", "HyperliquidTransaction:SendMultiSig", NonceKey::Nonce, nonce)
            .field("multiSigActionHash", Field::Bytes32(self.action_hash(nonce, vault, network)))
    }

    /// What the outer signer signs: `hyperliquidChain`, `multiSigActionHash`, then the nonce.
    #[must_use]
    pub fn typed(&self, nonce: u64, vault: Option<Address>, network: Network) -> TypedStruct<'static> {
        self.send(nonce, vault, network).typed(network)
    }

    /// What the outer signer's key signs: [`MultiSig::typed`] under the network's user-signed domain.
    #[must_use]
    pub fn digest(&self, nonce: u64, vault: Option<Address>, network: Network) -> [u8; 32] {
        self.send(nonce, vault, network).digest(network)
    }

    /// For the outer signer's wallet, on the network's signature chain.
    #[must_use]
    pub fn wallet_json(&self, nonce: u64, vault: Option<Address>, network: Network) -> String {
        self.send(nonce, vault, network).wallet_json(network)
    }

    /// The `/exchange` body, the outer signer's `signature` beside the action.
    #[must_use]
    pub fn body(&self, nonce: u64, signature: &Signature, vault: Option<Address>, network: Network) -> String {
        body::l1(&self.action(network), nonce, signature, vault)
    }
}

#[cfg(test)]
#[path = "tests/multi_sig.rs"]
mod tests;
