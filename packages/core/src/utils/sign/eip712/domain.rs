//! The domain a struct is signed under: the exchange's for an L1 action's phantom agent, or the
//! signature chain's for a user-signed action.

use crate::client::network::{L1_CHAIN_ID, Network};
use crate::utils::address::Address;
use crate::utils::json::Json;
use crate::utils::sign::eip712::word::{address_word, word};
use crate::utils::sign::keccak::{Keccak256, keccak256};

pub const DOMAIN_TYPE: &str = "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    pub name: &'static str,
    pub version: &'static str,
    pub chain_id: u64,
    pub verifying_contract: Address,
}

impl Domain {
    /// What an L1 action's phantom agent is signed under, on both networks.
    /// The chain id is the literal 1337, whatever chain the wallet is on; the
    /// network is in the phantom agent's `source`.
    pub const EXCHANGE: Domain =
        Domain { name: "Exchange", version: "1", chain_id: L1_CHAIN_ID, verifying_contract: Address::ZERO };

    /// What a user-signed action is signed under: the chain the action names
    /// as its `signatureChainId`.
    #[must_use]
    pub const fn user_signed(network: Network) -> Domain {
        Domain {
            name: "HyperliquidSignTransaction",
            version: "1",
            chain_id: network.signature_chain_id(),
            verifying_contract: Address::ZERO,
        }
    }

    #[must_use]
    pub fn separator(&self) -> [u8; 32] {
        Keccak256::new()
            .chain(&keccak256(DOMAIN_TYPE.as_bytes()))
            .chain(&keccak256(self.name.as_bytes()))
            .chain(&keccak256(self.version.as_bytes()))
            .chain(&word(self.chain_id))
            .chain(&address_word(self.verifying_contract))
            .finalize()
    }

    /// The `domain` object of wallet typed data: `chainId` a number, as viem sends it.
    pub(super) fn to_json(self) -> Json {
        Json::Object(vec![
            ("name".to_owned(), Json::String(self.name.to_owned())),
            ("version".to_owned(), Json::String(self.version.to_owned())),
            ("chainId".to_owned(), Json::Number(self.chain_id.to_string())),
            ("verifyingContract".to_owned(), Json::String(self.verifying_contract.to_hex())),
        ])
    }
}

#[cfg(test)]
#[path = "tests/domain.rs"]
mod tests;
