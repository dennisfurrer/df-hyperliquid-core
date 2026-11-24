//! `finalizeEvmContract`: the second half of linking a spot token to a HyperEVM ERC-20, sent by
//! the contract's deployer or the finalizer its storage names.

use crate::utils::sign::{L1Action, Map, Value};

/// How the sender of [`FinalizeEvmContract`] shows it deployed the
/// contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvmFinalizer {
    /// `{"create":{"nonce"}}`: the contract was deployed from an account,
    /// which sends this with the EVM nonce it deployed at.
    Create {
        /// The EVM nonce the deploying account used.
        nonce: u64,
    },
    /// `"firstStorageSlot"`: the contract's first storage slot holds the
    /// sender's address.
    FirstStorageSlot,
    /// `"customStorageSlot"`: the slot at `keccak256("HyperCore deployer")`
    /// holds the sender's address.
    CustomStorageSlot,
}

impl EvmFinalizer {
    /// The `input` as sent.
    #[must_use]
    pub fn wire(self) -> Value {
        match self {
            EvmFinalizer::Create { nonce } => Map::new().put("create", Map::new().put("nonce", nonce)).into(),
            EvmFinalizer::FirstStorageSlot => "firstStorageSlot".into(),
            EvmFinalizer::CustomStorageSlot => "customStorageSlot".into(),
        }
    }
}

/// `{"type":"finalizeEvmContract","token","input"}`: completes the link a
/// [`RequestEvmContract`](crate::deploy::spot::SpotDeploy::RequestEvmContract)
/// asked for. Not nested in `spotDeploy`. Linking cannot be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalizeEvmContract {
    /// The spot token index being linked.
    pub token: u32,
    /// How the sender shows it deployed the contract.
    pub input: EvmFinalizer,
}

impl L1Action for FinalizeEvmContract {
    fn action(&self) -> Map {
        Map::new().put("type", "finalizeEvmContract").put("token", self.token).put("input", self.input.wire())
    }
}
