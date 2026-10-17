//! Mainnet and testnet. Everything that differs between them is a method on
//! [`Network`]; an action signed for one is never valid on the other, so the
//! network is never assumed.

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

    /// The public websocket.
    #[must_use]
    pub const fn ws_url(self) -> &'static str {
        match self {
            Network::Mainnet => "wss://api.hyperliquid.xyz/ws",
            Network::Testnet => "wss://api.hyperliquid-testnet.xyz/ws",
        }
    }

    /// HyperEVM's JSON-RPC.
    #[must_use]
    pub const fn evm_rpc_url(self) -> &'static str {
        match self {
            Network::Mainnet => "https://rpc.hyperliquid.xyz/evm",
            Network::Testnet => "https://rpc.hyperliquid-testnet.xyz/evm",
        }
    }

    /// HyperEVM's chain id.
    #[must_use]
    pub const fn evm_chain_id(self) -> u64 {
        match self {
            Network::Mainnet => 999,
            Network::Testnet => 998,
        }
    }

    /// The chain every user-signed action is signed under and names as its
    /// `signatureChainId`: Arbitrum One on mainnet, Arbitrum Sepolia on
    /// testnet. The venue tells the networks apart by `hyperliquidChain`.
    #[must_use]
    pub const fn signature_chain_id(self) -> u64 {
        match self {
            Network::Mainnet => 42_161,
            Network::Testnet => 421_614,
        }
    }

    /// The Arbitrum chain the bridge is on, where a deposit goes. Separate
    /// from [`Network::signature_chain_id`], though the two agree today.
    #[must_use]
    pub const fn arbitrum_chain_id(self) -> u64 {
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

    /// Spot HYPE, whose index and token id differ between the networks.
    #[must_use]
    pub const fn hype(self) -> Token {
        match self {
            Network::Mainnet => MAINNET_HYPE,
            Network::Testnet => TESTNET_HYPE,
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

/// Spot USDC on mainnet, as `spotMeta` lists it.
pub const MAINNET_USDC: Token = Token {
    name: "USDC",
    index: USDC_INDEX,
    token_id: "0x6d1e7cde53ba9467b783cb7c530ce054",
    wire: "USDC:0x6d1e7cde53ba9467b783cb7c530ce054",
    wei_decimals: 8,
};

/// Spot USDC on testnet, as its `spotMeta` lists it.
pub const TESTNET_USDC: Token = Token {
    name: "USDC",
    index: USDC_INDEX,
    token_id: "0xeb62eee3685fc4c43992febcd9e75443",
    wire: "USDC:0xeb62eee3685fc4c43992febcd9e75443",
    wei_decimals: 8,
};

/// HYPE on mainnet, as `spotMeta` lists it.
pub const MAINNET_HYPE: Token = Token {
    name: "HYPE",
    index: 150,
    token_id: "0x0d01dc56dcaaca66ad901c959b4011ec",
    wire: "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec",
    wei_decimals: 8,
};

/// HYPE on testnet, as its `spotMeta` lists it. Testnet's token 150 is a
/// different token.
pub const TESTNET_HYPE: Token = Token {
    name: "HYPE",
    index: 1105,
    token_id: "0x7317beb7cceed72ef0b346074cc8e7ab",
    wire: "HYPE:0x7317beb7cceed72ef0b346074cc8e7ab",
    wei_decimals: 8,
};

#[cfg(test)]
#[path = "tests/network.rs"]
mod tests;
