//! Calldata and reads against the bridge's own addresses; answers as Arbitrum's node writes them.

use super::*;
use crate::client::evm::rpc::NodeError;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn each_network_deposits_through_its_own_bridge_and_usdc() {
    assert_eq!(bridge(Network::Mainnet).to_hex(), "0x2df1c51e09aecf9cacb7bc98cb1742757f163df7");
    assert_eq!(bridge(Network::Testnet).to_hex(), "0x08cfc1b6b2dcf36a1480b99353a354aa8ac56f89");
    assert_eq!(usdc(Network::Mainnet).to_hex(), "0xaf88d065e77c8cc2239327c5edb3a432268e5831");
    assert_eq!(usdc(Network::Testnet).to_hex(), "0x1baabb04529d43a73232b713c0fe471f7c7334d5");
    assert_eq!((Network::Mainnet.arbitrum_chain_id(), Network::Testnet.arbitrum_chain_id()), (42_161, 421_614));
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
    assert_eq!(usdc_transfer_calldata(bridge(Network::Mainnet), dec("-5")), Err(BadDeposit::NotPositive(dec("-5"))));
}

#[test]
fn the_reads_are_json_rpc_with_balance_of_calldata() {
    let owner: Address = "0x14791697260E4c9A71f18484C9f997B308e59325".parse().unwrap();
    assert_eq!(
        hex::encode_prefixed(&erc20::balance_of(owner)),
        "0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325"
    );
    assert_eq!(
        usdc_balance_request(Network::Mainnet, owner).body(),
        concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":"0xaf88d065e77c8cc2239327c5edb3a432268e5831","#,
            r#""data":"0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325"},"latest"]}"#
        )
    );
    assert!(
        usdc_balance_request(Network::Testnet, owner)
            .body()
            .contains(r#""to":"0x1baabb04529d43a73232b713c0fe471f7c7334d5""#)
    );
    assert_eq!(
        eth_balance_request(owner).body(),
        r#"{"jsonrpc":"2.0","id":1,"method":"eth_getBalance","params":["0x14791697260e4c9a71f18484c9f997b308e59325","latest"]}"#
    );
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
