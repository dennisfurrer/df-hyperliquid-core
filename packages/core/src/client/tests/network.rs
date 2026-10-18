//! Everything that differs between mainnet and testnet. Testnet's tokens were read from its
//! `spotMeta` on 2026-10-08; testnet's token 150 is a different token.

use super::*;
use crate::utils::sign::eip712::Domain;

#[test]
fn each_network_has_its_endpoints_chains_and_tokens() {
    struct Expected {
        api: &'static str,
        ws: &'static str,
        evm: &'static str,
        evm_chain: u64,
        signature_chain: u64,
        spelled: &'static str,
        chain: &'static str,
        source: &'static str,
        usdc: &'static str,
        hype: (u32, &'static str),
    }
    for (network, expected) in [
        (
            Network::Mainnet,
            Expected {
                api: "https://api.hyperliquid.xyz",
                ws: "wss://api.hyperliquid.xyz/ws",
                evm: "https://rpc.hyperliquid.xyz/evm",
                evm_chain: 999,
                signature_chain: 42_161,
                spelled: "0xa4b1",
                chain: "Mainnet",
                source: "a",
                usdc: "0x6d1e7cde53ba9467b783cb7c530ce054",
                hype: (150, "0x0d01dc56dcaaca66ad901c959b4011ec"),
            },
        ),
        (
            Network::Testnet,
            Expected {
                api: "https://api.hyperliquid-testnet.xyz",
                ws: "wss://api.hyperliquid-testnet.xyz/ws",
                evm: "https://rpc.hyperliquid-testnet.xyz/evm",
                evm_chain: 998,
                signature_chain: 421_614,
                spelled: "0x66eee",
                chain: "Testnet",
                source: "b",
                usdc: "0xeb62eee3685fc4c43992febcd9e75443",
                hype: (1105, "0x7317beb7cceed72ef0b346074cc8e7ab"),
            },
        ),
    ] {
        assert_eq!(
            (network.api_url(), network.ws_url(), network.evm_rpc_url()),
            (expected.api, expected.ws, expected.evm)
        );
        assert_eq!(network.evm_chain_id(), expected.evm_chain, "{network}");
        assert_eq!(
            (network.signature_chain_id(), network.arbitrum_chain_id()),
            (expected.signature_chain, expected.signature_chain)
        );
        assert_eq!(network.signature_chain_id_hex(), expected.spelled);
        assert_eq!(format!("{:#x}", network.signature_chain_id()), expected.spelled, "the id and its spelling agree");
        assert_eq!((network.hyperliquid_chain(), network.agent_source()), (expected.chain, expected.source));
        assert_eq!(Domain::user_signed(network).chain_id, expected.signature_chain, "user-signed under its chain");
        let (usdc, hype) = (network.usdc(), network.hype());
        assert_eq!((usdc.name, usdc.index, usdc.token_id, usdc.wei_decimals), ("USDC", 0, expected.usdc, 8));
        assert_eq!(
            (hype.name, hype.index, hype.token_id, hype.wei_decimals),
            ("HYPE", expected.hype.0, expected.hype.1, 8)
        );
        for token in [usdc, hype] {
            assert_eq!(token.wire, format!("{}:{}", token.name, token.token_id), "{network}");
        }
    }
    assert_eq!((Domain::EXCHANGE.chain_id, L1_CHAIN_ID), (1337, 1337), "an L1 action's on both");
    assert_eq!(Domain::user_signed(Network::Mainnet).name, "HyperliquidSignTransaction");
}

#[test]
fn a_network_reads_and_writes_its_name_and_mainnet_is_the_default() {
    assert_eq!("mainnet".parse::<Network>(), Ok(Network::Mainnet));
    assert_eq!("Testnet".parse::<Network>(), Ok(Network::Testnet));
    assert_eq!("devnet".parse::<Network>(), Err(UnknownNetwork("devnet".to_owned())));
    assert_eq!(Network::ALL.map(|network| network.to_string()), ["mainnet", "testnet"]);
    assert_eq!(Network::default(), Network::Mainnet);
}
