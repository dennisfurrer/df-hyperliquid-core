//! USDC in and out of Hyperliquid through the Arbitrum bridge. In: a USDC `transfer` to the bridge
//! on Arbitrum, credited to the same address on Hyperliquid in about a minute; under
//! [`MINIMUM_DEPOSIT`] it is lost. Out: `withdraw3` on the venue ([`crate::transfer::Withdraw3`]).
//! The reads before a deposit are [`crate::client::evm::rpc`] queries, sent by the caller.

use crate::client::Network;
use crate::client::evm::chain::NATIVE_DECIMALS;
use crate::client::evm::rpc::{BlockTag, CallRequest, Query, RpcError, eth};
use crate::client::evm::u256::U256;
use crate::transfer::erc20;
use crate::utils::{Address, Dec, USDC_DECIMALS, hex};

const fn address(text: &str) -> Address {
    match Address::parse_const(text) {
        Some(address) => address,
        None => Address::ZERO,
    }
}

/// The Hyperliquid bridge on Arbitrum One.
pub const MAINNET_BRIDGE: Address = address("0x2df1c51e09aecf9cacb7bc98cb1742757f163df7");
/// Testnet's bridge, on Arbitrum Sepolia.
pub const TESTNET_BRIDGE: Address = address("0x08cfc1B6b2dCF36A1480b99353A354AA8AC56f89");

/// Native USDC on Arbitrum One.
pub const MAINNET_USDC: Address = address("0xaf88d065e77c8cC2239327C5EDb3A432268e5831");
/// The USDC testnet's bridge takes, on Arbitrum Sepolia.
pub const TESTNET_USDC: Address = address("0x1baAbB04529D43a73232B713C0FE471f7c7334d5");

/// Below this the bridge credits nothing and the USDC is gone.
pub const MINIMUM_DEPOSIT: Dec = Dec::from_int(5);

/// The bridge a deposit on `network` goes to.
#[must_use]
pub const fn bridge(network: Network) -> Address {
    match network {
        Network::Mainnet => MAINNET_BRIDGE,
        Network::Testnet => TESTNET_BRIDGE,
    }
}

/// The USDC contract a deposit on `network` is a transfer of.
#[must_use]
pub const fn usdc(network: Network) -> Address {
    match network {
        Network::Mainnet => MAINNET_USDC,
        Network::Testnet => TESTNET_USDC,
    }
}

/// A public JSON-RPC for the Arbitrum chain the bridge is on ([`Network::arbitrum_chain_id`]).
#[must_use]
pub const fn rpc_url(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "https://arb1.arbitrum.io/rpc",
        Network::Testnet => "https://sepolia-rollup.arbitrum.io/rpc",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDeposit {
    #[error("{0} USDC has more than six decimal places")]
    TooFine(Dec),
    #[error("{0} USDC is not a positive amount")]
    NotPositive(Dec),
    #[error("{0} USDC is below the bridge's minimum of {MINIMUM_DEPOSIT}; less is lost")]
    BelowMinimum(Dec),
    #[error("{0} USDC is too large")]
    TooLarge(Dec),
}

/// `0x` calldata for USDC `transfer(to, amount)` ([`erc20::transfer`]). The amount is scaled to
/// six decimals exactly; a seventh place is refused, never rounded.
pub fn usdc_transfer_calldata(to: Address, amount: Dec) -> Result<String, BadDeposit> {
    if !amount.is_positive() {
        return Err(BadDeposit::NotPositive(amount));
    }
    if amount.places() > USDC_DECIMALS {
        return Err(BadDeposit::TooFine(amount));
    }
    let units = U256::from_dec(amount, USDC_DECIMALS).map_err(|_| BadDeposit::TooLarge(amount))?;
    Ok(hex::encode_prefixed(&erc20::transfer(to, units)))
}

/// [`usdc_transfer_calldata`] to the network's bridge, refusing what it would lose. The
/// transaction goes to [`usdc`] on [`Network::arbitrum_chain_id`] with no value.
pub fn deposit_calldata(network: Network, amount: Dec) -> Result<String, BadDeposit> {
    if amount.is_positive() && amount < MINIMUM_DEPOSIT {
        return Err(BadDeposit::BelowMinimum(amount));
    }
    usdc_transfer_calldata(bridge(network), amount)
}

/// The owner's USDC on the network's Arbitrum chain: `balanceOf` through `eth_call` at the latest
/// block. Its answer reads with [`usdc_amount`].
#[must_use]
pub fn usdc_balance_request(network: Network, owner: Address) -> Query<Vec<u8>> {
    eth::call(&CallRequest::to(usdc(network), erc20::balance_of(owner)), BlockTag::Latest)
}

/// The owner's ETH, for gas, at the latest block. Its answer reads with [`eth_amount`].
#[must_use]
pub fn eth_balance_request(owner: Address) -> Query<U256> {
    eth::balance(owner, BlockTag::Latest)
}

/// A [`usdc_balance_request`] answer as USDC, exactly.
pub fn usdc_amount(answer: &[u8]) -> Result<Dec, RpcError> {
    let unreadable = |detail: String| RpcError::Unreadable { method: "eth_call", detail };
    let units = erc20::read_amount(answer).map_err(|error| unreadable(error.to_string()))?;
    units.to_dec(USDC_DECIMALS).map_err(|error| unreadable(error.to_string()))
}

/// An [`eth_balance_request`] answer as ETH, exactly.
pub fn eth_amount(wei: U256) -> Result<Dec, RpcError> {
    wei.to_dec(NATIVE_DECIMALS)
        .map_err(|error| RpcError::Unreadable { method: "eth_getBalance", detail: error.to_string() })
}

#[cfg(test)]
#[path = "tests/bridge.rs"]
mod tests;
