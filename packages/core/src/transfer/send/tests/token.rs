//! `spot_tokens.json` is live `spotMeta`, mainnet and testnet, 2026-10-08, cut to the tokens
//! below, each as the venue wrote it: which is the network's HYPE, and where and at how many places
//! each token crosses to HyperEVM.

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::Address;

use super::*;
use crate::transfer::evm::{HYPE_SYSTEM, system_address};

const TOKENS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/transfer/spot_tokens.json"));

fn token(network: Network, name: &str) -> SpotToken {
    let all: BTreeMap<String, Vec<SpotToken>> = serde_json::from_str(TOKENS).unwrap();
    all[network.name()].iter().find(|token| token.name == name).cloned().unwrap()
}

/// HYPE's index and id differ between the networks: 150 on mainnet, 1105 on testnet, where 150 is
/// another token altogether. USDC is named by each network's own id.
#[test]
fn hype_is_the_network_s_own_token() {
    for network in Network::ALL {
        let listed = token(network, "HYPE");
        let ours = network.hype();
        assert_eq!(
            (listed.index, listed.token_id.as_str(), listed.wire(), listed.wei_decimals),
            (ours.index, ours.token_id, ours.wire.to_owned(), ours.wei_decimals),
            "{network}"
        );
        assert!(listed.is_hype(network), "{network}");
        let usdc = token(network, "USDC");
        assert_eq!(usdc.wire(), network.usdc().wire, "{network}");
        assert_eq!(SpotToken::from(network.usdc()).wire(), usdc.wire(), "{network}");
    }
    assert_eq!((Network::Mainnet.hype().index, Network::Testnet.hype().index), (150, 1105));
    let testnet_150 = token(Network::Testnet, "ALERTT");
    assert_eq!(testnet_150.index, 150);
    assert!(!testnet_150.is_hype(Network::Testnet));
    assert!(!token(Network::Mainnet, "HYPE").is_hype(Network::Testnet));
}

/// HYPE is HyperEVM's own coin, eighteen decimals, crossing through `0x2222...2222`: on mainnet
/// `spotMeta` links it to nothing, on testnet to the zero address with ten more decimals. Every
/// other token crosses through `0x20` and its index, at its ERC-20's `weiDecimals` plus the extra,
/// or not at all. KNTQ's address is the destination of a live `spotTransfer` of KNTQ to HyperEVM in
/// `ledger.json`.
#[test]
fn a_token_crosses_through_its_own_address_keeping_the_fewer_of_its_two_sides_places() {
    let mainnet = |name: &str| token(Network::Mainnet, name);
    let testnet = |name: &str| token(Network::Testnet, name);
    assert_eq!(mainnet("HYPE").evm_contract, None);
    assert_eq!(
        testnet("HYPE").evm_contract.map(|link| (link.address, link.extra_wei_decimals)),
        Some((Address::ZERO, 10))
    );
    for (network, token, through, evm, crossing) in [
        (Network::Mainnet, mainnet("HYPE"), HYPE_SYSTEM, Some(18), Some(8)),
        (Network::Testnet, testnet("HYPE"), HYPE_SYSTEM, Some(18), Some(8)),
        (Network::Mainnet, mainnet("UBTC"), system_address(197), Some(8), Some(8)),
        (
            Network::Mainnet,
            mainnet("KNTQ"),
            "0x200000000000000000000000000000000000007c".parse().unwrap(),
            Some(18),
            Some(8),
        ),
        (Network::Mainnet, mainnet("USDC"), system_address(0), Some(6), Some(6)),
        (Network::Testnet, testnet("USDC"), system_address(0), Some(6), Some(6)),
        (Network::Testnet, testnet("ALERTT"), system_address(150), None, None),
    ] {
        assert_eq!(token.system_address(network), through, "{network} {}", token.name);
        assert_eq!(token.evm_decimals(network), evm, "{network} {}", token.name);
        assert_eq!(token.crossing_places(network), crossing, "{network} {}", token.name);
    }
}
