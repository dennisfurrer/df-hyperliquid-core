//! The four EVM chains a Hyperliquid account touches: HyperEVM and the Arbitrum
//! chain its bridge is on, for each network. HyperEVM's RPC comes from
//! [`Network::evm_rpc_url`]; an Arbitrum RPC is whatever URL the caller has.

use crate::client::Network;

pub const ARBITRUM_ONE: u64 = 42_161;
pub const ARBITRUM_SEPOLIA: u64 = 421_614;

/// HYPE on HyperEVM and ETH on Arbitrum both have 18 decimals.
pub const NATIVE_DECIMALS: u32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chain {
    HyperEvm(Network),
    Arbitrum(Network),
}

impl Chain {
    #[must_use]
    pub const fn id(self) -> u64 {
        match self {
            Chain::HyperEvm(network) => network.evm_chain_id(),
            Chain::Arbitrum(network) => network.arbitrum_chain_id(),
        }
    }

    #[must_use]
    pub fn from_id(id: u64) -> Option<Chain> {
        [Network::Mainnet, Network::Testnet]
            .into_iter()
            .flat_map(|network| [Chain::HyperEvm(network), Chain::Arbitrum(network)])
            .find(|chain| chain.id() == id)
    }

    #[must_use]
    pub const fn network(self) -> Network {
        match self {
            Chain::HyperEvm(network) | Chain::Arbitrum(network) => network,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Chain::HyperEvm(Network::Mainnet) => "HyperEVM",
            Chain::HyperEvm(Network::Testnet) => "HyperEVM testnet",
            Chain::Arbitrum(Network::Mainnet) => "Arbitrum One",
            Chain::Arbitrum(Network::Testnet) => "Arbitrum Sepolia",
        }
    }

    /// The coin gas is paid in.
    #[must_use]
    pub const fn coin(self) -> &'static str {
        match self {
            Chain::HyperEvm(_) => "HYPE",
            Chain::Arbitrum(_) => "ETH",
        }
    }
}

#[cfg(test)]
#[path = "tests/chain.rs"]
mod tests;
