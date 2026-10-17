//! A spot token as a transfer names it, read from the caller's `spotMeta`, and HYPE, whose index
//! and id differ between the networks.

use crate::client::{Network, Token};
use crate::transfer::check::{HYPE_SYSTEM, system_address};
use crate::utils::Address;
use serde::Deserialize;

/// HYPE on each network.
pub use crate::client::{MAINNET_HYPE, TESTNET_HYPE};

/// HYPE's decimals on HyperEVM, where it is the chain's own coin.
pub const HYPE_EVM_DECIMALS: u32 = 18;

/// The network's HYPE: [`Network::hype`].
#[must_use]
pub const fn hype(network: Network) -> Token {
    network.hype()
}

/// One token of `spotMeta`'s `tokens`, the fields a transfer reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotToken {
    pub name: String,
    /// What a balance's `token` and `borrowLend` name it by.
    pub index: u32,
    /// `0x` and 32 hex digits.
    pub token_id: String,
    /// The decimals Core counts it in.
    pub wei_decimals: u32,
    /// Its ERC-20 on HyperEVM; `null` for a token with none, and for HYPE on
    /// mainnet, which is HyperEVM's own coin.
    #[serde(default)]
    pub evm_contract: Option<EvmContract>,
}

/// A token's ERC-20 on HyperEVM. The venue spells this key in snake case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct EvmContract {
    pub address: Address,
    /// Its decimals on HyperEVM less `weiDecimals`.
    #[serde(rename = "evm_extra_wei_decimals")]
    pub extra_wei_decimals: i32,
}

impl SpotToken {
    /// `NAME:0x<tokenId>`, the one way a transfer names a token.
    #[must_use]
    pub fn wire(&self) -> String {
        format!("{}:{}", self.name, self.token_id)
    }

    /// Whether this is the network's HYPE, by its index there.
    #[must_use]
    pub fn is_hype(&self, network: Network) -> bool {
        self.index == network.hype().index
    }

    /// Where it crosses from Core to HyperEVM: `0x2222...2222` for HYPE, `0x20` and the index for
    /// any other token.
    #[must_use]
    pub fn system_address(&self, network: Network) -> Address {
        match self.is_hype(network) {
            true => HYPE_SYSTEM,
            false => system_address(u64::from(self.index)),
        }
    }

    /// Its decimals on HyperEVM: eighteen for HYPE, `weiDecimals` plus the
    /// ERC-20's extra for a linked token, `None` for a token with no ERC-20.
    #[must_use]
    pub fn evm_decimals(&self, network: Network) -> Option<u32> {
        if self.is_hype(network) {
            return Some(HYPE_EVM_DECIMALS);
        }
        let link = self.evm_contract?;
        u32::try_from(i64::from(self.wei_decimals) + i64::from(link.extra_wei_decimals)).ok()
    }

    /// The most places an amount crossing to HyperEVM may have: the fewer of its Core and HyperEVM
    /// decimals. `None` when it cannot cross.
    #[must_use]
    pub fn crossing_places(&self, network: Network) -> Option<u32> {
        self.evm_decimals(network).map(|evm| evm.min(self.wei_decimals))
    }
}

/// USDC or HYPE from the network itself, with no link to HyperEVM read: HYPE
/// needs none, and USDC does not cross here.
impl From<Token> for SpotToken {
    fn from(token: Token) -> SpotToken {
        SpotToken {
            name: token.name.to_owned(),
            index: token.index,
            token_id: token.token_id.to_owned(),
            wei_decimals: token.wei_decimals,
            evm_contract: None,
        }
    }
}

#[cfg(test)]
#[path = "tests/token.rs"]
mod tests;
