//! Calldata against the bridge's own addresses, what it would lose refused, and answers as
//! Arbitrum's node writes them. The reads' bodies are held in `tests/mocked/transfer.rs`.

use super::*;
use crate::client::evm::rpc::NodeError;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// Each network's bridge and USDC on its own Arbitrum chain; the amount in millionths, exactly.
#[test]
fn a_deposit_is_a_usdc_transfer_to_the_network_s_bridge_in_millionths() {
    for (network, bridge_at, usdc_at, chain, rpc) in [
        (
            Network::Mainnet,
            "0x2df1c51e09aecf9cacb7bc98cb1742757f163df7",
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831",
            42_161,
            "https://arb1.arbitrum.io/rpc",
        ),
        (
            Network::Testnet,
            "0x08cfc1b6b2dcf36a1480b99353a354aa8ac56f89",
            "0x1baabb04529d43a73232b713c0fe471f7c7334d5",
            421_614,
            "https://sepolia-rollup.arbitrum.io/rpc",
        ),
    ] {
        assert_eq!((bridge(network).to_hex(), usdc(network).to_hex()), (bridge_at.to_owned(), usdc_at.to_owned()));
        assert_eq!((network.arbitrum_chain_id(), rpc_url(network)), (chain, rpc), "{network}");
        assert_eq!(
            deposit_calldata(network, dec("12.5")).unwrap(),
            format!(
                "0xa9059cbb000000000000000000000000{}0000000000000000000000000000000000000000000000000000000000bebc20",
                &bridge_at[2..]
            ),
            "{network}"
        );
    }
    assert_eq!(
        usdc_transfer_calldata(Address::ZERO, dec("0.000001")).unwrap(),
        concat!(
            "0xa9059cbb",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
    );
    assert_eq!(MINIMUM_DEPOSIT, dec("5"));
    assert_eq!(deposit_calldata(Network::Mainnet, dec("5")).unwrap().len(), 2 + 8 + 64 + 64);
    assert!(
        usdc_balance_request(Network::Testnet, Address::ZERO)
            .body()
            .contains(r#""to":"0x1baabb04529d43a73232b713c0fe471f7c7334d5""#)
    );
}

#[test]
fn what_the_bridge_would_lose_or_round_is_refused() {
    for network in Network::ALL {
        assert_eq!(deposit_calldata(network, dec("4.999999")), Err(BadDeposit::BelowMinimum(dec("4.999999"))));
        assert_eq!(deposit_calldata(network, dec("5.0000001")), Err(BadDeposit::TooFine(dec("5.0000001"))));
        assert_eq!(deposit_calldata(network, Dec::ZERO), Err(BadDeposit::NotPositive(Dec::ZERO)));
    }
    assert_eq!(usdc_transfer_calldata(bridge(Network::Mainnet), dec("-5")), Err(BadDeposit::NotPositive(dec("-5"))));
}

#[test]
fn answers_read_as_exact_amounts() {
    let owner = Address::ZERO;
    let usdc_of =
        |answer: &str| usdc_balance_request(Network::Mainnet, owner).answer(answer).and_then(|data| usdc_amount(&data));
    let eth_of = |answer: &str| eth_balance_request(owner).answer(answer).and_then(eth_amount);
    let word =
        r#"{"jsonrpc":"2.0","id":1,"result":"0x00000000000000000000000000000000000000000000000000000000004c4b40"}"#;
    assert_eq!(usdc_of(word), Ok(dec("5")));
    assert_eq!(eth_of(r#"{"jsonrpc":"2.0","id":1,"result":"0x2386f26fc10000"}"#), Ok(dec("0.01")));
    assert_eq!(eth_of(r#"{"jsonrpc":"2.0","id":1,"result":"0x0"}"#), Ok(Dec::ZERO));
    assert_eq!(
        usdc_of(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"execution reverted"}}"#),
        Err(RpcError::Node {
            method: "eth_call",
            error: NodeError { code: -32000, message: "execution reverted".to_owned(), data: None },
        })
    );
    assert!(usdc_of(r#"{"jsonrpc":"2.0","id":1,"result":"12"}"#).is_err());
    assert!(usdc_of(r#"{"jsonrpc":"2.0","id":1,"result":"0x"}"#).is_err(), "no word is no balance");
    assert!(usdc_of("<html>").is_err());
    assert!(eth_amount(U256::MAX).is_err(), "past what a Dec counts");
}
