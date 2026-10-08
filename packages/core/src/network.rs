//! The two networks. Everything that differs between them is a method on
//! [`Network`], and every signature takes one: an action signed for testnet
//! is never a valid mainnet action, and the other way round, so the network
//! is never assumed.

use std::{fmt, str::FromStr};

/// The chain id an L1 action's domain carries on both networks: a literal,
/// not a real chain.
pub const L1_CHAIN_ID: u64 = 1337;

/// Spot USDC's index on both networks.
pub const USDC_INDEX: u32 = 0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Network {
    #[default]
    Mainnet,
    Testnet,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a network; expected mainnet or testnet")]
pub struct UnknownNetwork(pub String);

impl Network {
    pub const ALL: [Network; 2] = [Network::Mainnet, Network::Testnet];

    /// Both endpoints live under it: `/info` and `/exchange`.
    #[must_use]
    pub const fn api_url(self) -> &'static str {
        match self {
            Network::Mainnet => "https://api.hyperliquid.xyz",
            Network::Testnet => "https://api.hyperliquid-testnet.xyz",
        }
    }

    /// Arbitrum One on mainnet, Arbitrum Sepolia on testnet. Every
    /// user-signed action is signed under this chain and names it as its
    /// `signatureChainId`. The venue accepts any chain a signature names and
    /// tells the networks apart by `hyperliquidChain`; one rule here keeps a
    /// signature and the chain it names from disagreeing.
    #[must_use]
    pub const fn signature_chain_id(self) -> u64 {
        match self {
            Network::Mainnet => 42_161,
            Network::Testnet => 421_614,
        }
    }

    /// [`Network::signature_chain_id`] as an action spells it.
    #[must_use]
    pub const fn signature_chain_id_hex(self) -> &'static str {
        match self {
            Network::Mainnet => "0xa4b1",
            Network::Testnet => "0x66eee",
        }
    }

    /// What a user-signed action names as its `hyperliquidChain`.
    #[must_use]
    pub const fn hyperliquid_chain(self) -> &'static str {
        match self {
            Network::Mainnet => "Mainnet",
            Network::Testnet => "Testnet",
        }
    }

    /// The phantom agent's `source` for an L1 action.
    #[must_use]
    pub const fn agent_source(self) -> &'static str {
        match self {
            Network::Mainnet => "a",
            Network::Testnet => "b",
        }
    }

    /// Spot USDC, whose token id differs between the networks.
    #[must_use]
    pub const fn usdc(self) -> Token {
        match self {
            Network::Mainnet => MAINNET_USDC,
            Network::Testnet => TESTNET_USDC,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Testnet => "testnet",
        }
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Network {
    type Err = UnknownNetwork;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "mainnet" => Ok(Network::Mainnet),
            "testnet" => Ok(Network::Testnet),
            _ => Err(UnknownNetwork(text.to_owned())),
        }
    }
}

/// A spot token as the venue names it in each place it appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    pub name: &'static str,
    /// `token` in `spotClearinghouseState`, and the index in `spotMeta`.
    pub index: u32,
    pub token_id: &'static str,
    /// `NAME:0x<tokenId>`, how a transfer names the token.
    pub wire: &'static str,
    /// The decimals the venue counts the token in.
    pub wei_decimals: u32,
}

/// Spot USDC on mainnet, checked against `spotMeta` on 2026-10-06.
pub const MAINNET_USDC: Token = Token {
    name: "USDC",
    index: USDC_INDEX,
    token_id: "0x6d1e7cde53ba9467b783cb7c530ce054",
    wire: "USDC:0x6d1e7cde53ba9467b783cb7c530ce054",
    wei_decimals: 8,
};

/// Spot USDC on testnet, checked against its `spotMeta` on 2026-10-08.
pub const TESTNET_USDC: Token = Token {
    name: "USDC",
    index: USDC_INDEX,
    token_id: "0xeb62eee3685fc4c43992febcd9e75443",
    wire: "USDC:0xeb62eee3685fc4c43992febcd9e75443",
    wei_decimals: 8,
};

#[cfg(test)]
#[path = "tests/network.rs"]
mod tests;
