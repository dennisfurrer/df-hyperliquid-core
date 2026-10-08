//! USDC into Hyperliquid through the Arbitrum bridge: the calldata the wallet
//! sends and the JSON-RPC reads before it. Pure; the app posts them.
//!
//! A plain USDC `transfer` to the bridge is credited to the same address on
//! Hyperliquid in about a minute. Under [`MINIMUM_DEPOSIT`] it is lost.
//! Mainnet deposits on Arbitrum One, testnet on Arbitrum Sepolia, each with
//! its own bridge and USDC (hip4-tui docs/WIRE.md).

use hl_core::{Address, Dec, Json, Network, Round, hex};
use serde_json::Value;

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

/// Arbitrum One or Arbitrum Sepolia: the chain user-signed actions name too.
#[must_use]
pub const fn chain_id(network: Network) -> u64 {
    network.signature_chain_id()
}

/// A public JSON-RPC for that chain.
#[must_use]
pub const fn rpc_url(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "https://arb1.arbitrum.io/rpc",
        Network::Testnet => "https://sepolia-rollup.arbitrum.io/rpc",
    }
}

pub const USDC_DECIMALS: u32 = 6;
pub const ETH_DECIMALS: u32 = 18;

/// Below this the bridge credits nothing and the USDC is gone.
pub const MINIMUM_DEPOSIT: Dec = Dec::from_int(5);

/// `transfer(address,uint256)`.
const TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
/// `balanceOf(address)`.
const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpcError {
    /// The node's `error.message`.
    #[error("the node refused: {0}")]
    Refused(String),
    #[error("the node's answer could not be read: {0}")]
    Unreadable(String),
}

/// `0x` calldata for USDC `transfer(to, amount)`: the selector, then each
/// argument as a 32-byte word. The amount is scaled to six decimals exactly;
/// a seventh place is refused, never rounded.
pub fn usdc_transfer_calldata(to: Address, amount: Dec) -> Result<String, BadDeposit> {
    if !amount.is_positive() {
        return Err(BadDeposit::NotPositive(amount));
    }
    if amount.places() > USDC_DECIMALS {
        return Err(BadDeposit::TooFine(amount));
    }
    let units = amount
        .units_at(USDC_DECIMALS, Round::Down)
        .and_then(|units| u128::try_from(units).ok())
        .ok_or(BadDeposit::TooLarge(amount))?;
    let mut data = Vec::with_capacity(68);
    data.extend_from_slice(&TRANSFER);
    data.extend_from_slice(&address_word(to));
    data.extend_from_slice(&uint_word(units));
    Ok(hex::encode_prefixed(&data))
}

/// [`usdc_transfer_calldata`] to the network's bridge, refusing what it
/// would lose. The transaction goes to [`usdc`] on [`chain_id`] with no value.
pub fn deposit_calldata(network: Network, amount: Dec) -> Result<String, BadDeposit> {
    if amount.is_positive() && amount < MINIMUM_DEPOSIT {
        return Err(BadDeposit::BelowMinimum(amount));
    }
    usdc_transfer_calldata(bridge(network), amount)
}

/// `0x` calldata for `balanceOf(owner)`.
#[must_use]
pub fn balance_of_calldata(owner: Address) -> String {
    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(&BALANCE_OF);
    data.extend_from_slice(&address_word(owner));
    hex::encode_prefixed(&data)
}

/// `eth_call` of `data` on `to` at the latest block.
#[must_use]
pub fn eth_call_request(to: Address, data: &str) -> String {
    let call = Json::Object(vec![
        ("to".to_owned(), Json::String(to.to_hex())),
        ("data".to_owned(), Json::String(data.to_owned())),
    ]);
    rpc("eth_call", vec![call, Json::String("latest".to_owned())])
}

/// The owner's USDC on the network's chain, read with `balanceOf`; its answer
/// reads with [`read_quantity`] at [`USDC_DECIMALS`].
#[must_use]
pub fn usdc_balance_request(network: Network, owner: Address) -> String {
    eth_call_request(usdc(network), &balance_of_calldata(owner))
}

/// The owner's ETH, for gas; its answer reads at [`ETH_DECIMALS`].
#[must_use]
pub fn eth_balance_request(owner: Address) -> String {
    rpc("eth_getBalance", vec![Json::String(owner.to_hex()), Json::String("latest".to_owned())])
}

fn rpc(method: &str, params: Vec<Json>) -> String {
    Json::Object(vec![
        ("jsonrpc".to_owned(), Json::String("2.0".to_owned())),
        ("id".to_owned(), Json::Number("1".to_owned())),
        ("method".to_owned(), Json::String(method.to_owned())),
        ("params".to_owned(), Json::Array(params)),
    ])
    .compact()
}

/// A JSON-RPC answer's `result`, a hex quantity or 32-byte word, as an amount
/// with `decimals` places.
pub fn read_quantity(answer: &str, decimals: u32) -> Result<Dec, RpcError> {
    let said: Value = serde_json::from_str(answer).map_err(|error| RpcError::Unreadable(error.to_string()))?;
    if let Some(error) = said.get("error") {
        let message = error.get("message").and_then(Value::as_str).map_or_else(|| error.to_string(), str::to_owned);
        return Err(RpcError::Refused(message));
    }
    let result = said.get("result").and_then(Value::as_str).ok_or_else(|| RpcError::Unreadable(said.to_string()))?;
    quantity(result, decimals).ok_or_else(|| RpcError::Unreadable(result.to_owned()))
}

/// `0x`-prefixed hex as an amount with `decimals` places: `0x4c4b40` at six is
/// `5`. `None` for anything else, or past 128 bits.
#[must_use]
pub fn quantity(text: &str, decimals: u32) -> Option<Dec> {
    let digits = text.strip_prefix("0x")?.trim_start_matches('0');
    let units = match digits.is_empty() {
        true => 0,
        false if digits.len() <= 32 => u128::from_str_radix(digits, 16).ok()?,
        false => return None,
    };
    Dec::new(i128::try_from(units).ok()?, decimals)
}

fn address_word(address: Address) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(address.bytes());
    word
}

fn uint_word(value: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Dec {
        Dec::parse(text).unwrap()
    }

    /// hip4-tui docs/WIRE.md, "HyperEVM and Arbitrum transactions": Arbitrum
    /// One on mainnet, Arbitrum Sepolia on testnet, each with its own bridge
    /// and USDC.
    #[test]
    fn each_network_deposits_through_its_own_bridge_and_usdc() {
        assert_eq!(bridge(Network::Mainnet).to_hex(), "0x2df1c51e09aecf9cacb7bc98cb1742757f163df7");
        assert_eq!(bridge(Network::Testnet).to_hex(), "0x08cfc1b6b2dcf36a1480b99353a354aa8ac56f89");
        assert_eq!(usdc(Network::Mainnet).to_hex(), "0xaf88d065e77c8cc2239327c5edb3a432268e5831");
        assert_eq!(usdc(Network::Testnet).to_hex(), "0x1baabb04529d43a73232b713c0fe471f7c7334d5");
        assert_eq!((chain_id(Network::Mainnet), chain_id(Network::Testnet)), (42_161, 421_614));
        assert_eq!(rpc_url(Network::Mainnet), "https://arb1.arbitrum.io/rpc");
        assert_eq!(rpc_url(Network::Testnet), "https://sepolia-rollup.arbitrum.io/rpc");
        assert_eq!(MINIMUM_DEPOSIT, dec("5"));
    }

    #[test]
    fn a_deposit_is_a_usdc_transfer_to_the_bridge_in_millionths() {
        assert_eq!(
            deposit_calldata(Network::Mainnet, dec("12.5")).unwrap(),
            concat!(
                "0xa9059cbb",
                "0000000000000000000000002df1c51e09aecf9cacb7bc98cb1742757f163df7",
                "0000000000000000000000000000000000000000000000000000000000bebc20",
            )
        );
        assert_eq!(
            deposit_calldata(Network::Testnet, dec("12.5")).unwrap(),
            concat!(
                "0xa9059cbb",
                "00000000000000000000000008cfc1b6b2dcf36a1480b99353a354aa8ac56f89",
                "0000000000000000000000000000000000000000000000000000000000bebc20",
            )
        );
        assert_eq!(
            usdc_transfer_calldata(Address::ZERO, dec("0.000001")).unwrap(),
            concat!(
                "0xa9059cbb",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
        );
        assert_eq!(deposit_calldata(Network::Mainnet, dec("5")).unwrap().len(), 2 + 8 + 64 + 64);
    }

    #[test]
    fn what_the_bridge_would_lose_or_round_is_refused() {
        for network in Network::ALL {
            assert_eq!(deposit_calldata(network, dec("4.999999")), Err(BadDeposit::BelowMinimum(dec("4.999999"))));
            assert_eq!(deposit_calldata(network, dec("5.0000001")), Err(BadDeposit::TooFine(dec("5.0000001"))));
            assert_eq!(deposit_calldata(network, Dec::ZERO), Err(BadDeposit::NotPositive(Dec::ZERO)));
        }
        assert_eq!(
            usdc_transfer_calldata(bridge(Network::Mainnet), dec("-5")),
            Err(BadDeposit::NotPositive(dec("-5")))
        );
    }

    #[test]
    fn the_reads_are_json_rpc_with_balance_of_calldata() {
        let owner: Address = "0x14791697260E4c9A71f18484C9f997B308e59325".parse().unwrap();
        assert_eq!(
            balance_of_calldata(owner),
            "0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325"
        );
        assert_eq!(
            usdc_balance_request(Network::Mainnet, owner),
            concat!(
                r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":"0xaf88d065e77c8cc2239327c5edb3a432268e5831","#,
                r#""data":"0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325"},"latest"]}"#
            )
        );
        assert!(
            usdc_balance_request(Network::Testnet, owner)
                .contains(r#""to":"0x1baabb04529d43a73232b713c0fe471f7c7334d5""#)
        );
        assert_eq!(
            eth_balance_request(owner),
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_getBalance","params":["0x14791697260e4c9a71f18484c9f997b308e59325","latest"]}"#
        );
    }

    #[test]
    fn answers_read_as_exact_amounts() {
        let word =
            r#"{"jsonrpc":"2.0","id":1,"result":"0x00000000000000000000000000000000000000000000000000000000004c4b40"}"#;
        assert_eq!(read_quantity(word, USDC_DECIMALS), Ok(dec("5")));
        let wei = r#"{"jsonrpc":"2.0","id":1,"result":"0x2386f26fc10000"}"#;
        assert_eq!(read_quantity(wei, ETH_DECIMALS), Ok(dec("0.01")));
        assert_eq!(read_quantity(r#"{"jsonrpc":"2.0","id":1,"result":"0x0"}"#, 18), Ok(Dec::ZERO));
        assert_eq!(read_quantity(r#"{"jsonrpc":"2.0","id":1,"result":"0x"}"#, 6), Ok(Dec::ZERO));
        assert_eq!(
            read_quantity(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"execution reverted"}}"#, 6),
            Err(RpcError::Refused("execution reverted".to_owned()))
        );
        assert!(read_quantity(r#"{"jsonrpc":"2.0","id":1,"result":"12"}"#, 6).is_err());
        assert!(read_quantity("<html>", 6).is_err());
        assert_eq!(quantity(&format!("0x1{}", "0".repeat(32)), 6), None, "past 128 bits");
    }
}
