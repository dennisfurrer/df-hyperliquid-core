//! Chain ids as Hyperliquid's docs give them and the chains answer
//! `eth_chainId` (HyperEVM mainnet's live answer is `0x3e7`).

use serde_json::Value;

use super::*;
use crate::client::evm::fixtures;

#[test]
fn each_network_has_its_hyperevm_and_its_arbitrum() {
    assert_eq!(Chain::HyperEvm(Network::Mainnet).id(), 999);
    assert_eq!(Chain::HyperEvm(Network::Testnet).id(), 998);
    assert_eq!(Chain::Arbitrum(Network::Mainnet).id(), ARBITRUM_ONE);
    assert_eq!(Chain::Arbitrum(Network::Testnet).id(), ARBITRUM_SEPOLIA);
    assert_eq!((ARBITRUM_ONE, ARBITRUM_SEPOLIA), (42_161, 421_614));
    for network in Network::ALL {
        assert_eq!(Chain::HyperEvm(network).id(), network.evm_chain_id(), "{network}");
        assert_eq!(Chain::Arbitrum(network).id(), network.signature_chain_id(), "{network}");
    }
    let live: Value = serde_json::from_str(&fixtures::answer("eth_chainId", 0)).unwrap();
    assert_eq!(live["result"], "0x3e7");
}

#[test]
fn a_chain_is_named_and_found_by_its_id() {
    assert_eq!(Chain::Arbitrum(Network::Mainnet).name(), "Arbitrum One");
    assert_eq!(Chain::Arbitrum(Network::Testnet).name(), "Arbitrum Sepolia");
    assert_eq!(Chain::HyperEvm(Network::Mainnet).name(), "HyperEVM");
    assert_eq!(Chain::HyperEvm(Network::Testnet).name(), "HyperEVM testnet");
    assert_eq!(Chain::HyperEvm(Network::Mainnet).coin(), "HYPE");
    assert_eq!(Chain::Arbitrum(Network::Testnet).coin(), "ETH");
    assert_eq!(Chain::HyperEvm(Network::Mainnet).network(), Network::Mainnet);
    for chain in [
        Chain::HyperEvm(Network::Mainnet),
        Chain::HyperEvm(Network::Testnet),
        Chain::Arbitrum(Network::Mainnet),
        Chain::Arbitrum(Network::Testnet),
    ] {
        assert_eq!(Chain::from_id(chain.id()), Some(chain));
    }
    assert_eq!(Chain::from_id(1), None);
    assert_eq!(NATIVE_DECIMALS, 18);
}
