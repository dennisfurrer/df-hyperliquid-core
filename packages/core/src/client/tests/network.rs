use super::*;
use crate::utils::sign::eip712::Domain;

#[test]
fn mainnet_is_arbitrum_one_and_source_a() {
    let network = Network::Mainnet;
    assert_eq!(network.api_url(), "https://api.hyperliquid.xyz");
    assert_eq!(network.signature_chain_id(), 42_161);
    assert_eq!(network.signature_chain_id_hex(), "0xa4b1");
    assert_eq!(network.hyperliquid_chain(), "Mainnet");
    assert_eq!(network.agent_source(), "a");
    assert_eq!(network.usdc().wire, "USDC:0x6d1e7cde53ba9467b783cb7c530ce054");
}

/// Testnet's USDC was read from its `spotMeta` on 2026-10-08.
#[test]
fn testnet_is_arbitrum_sepolia_and_source_b() {
    let network = Network::Testnet;
    assert_eq!(network.api_url(), "https://api.hyperliquid-testnet.xyz");
    assert_eq!(network.signature_chain_id(), 421_614);
    assert_eq!(network.signature_chain_id_hex(), "0x66eee");
    assert_eq!(network.hyperliquid_chain(), "Testnet");
    assert_eq!(network.agent_source(), "b");
    assert_eq!(network.usdc().wire, "USDC:0xeb62eee3685fc4c43992febcd9e75443");
}

#[test]
fn each_chain_id_and_its_spelling_agree() {
    for network in Network::ALL {
        assert_eq!(format!("{:#x}", network.signature_chain_id()), network.signature_chain_id_hex(), "{network}");
    }
}

#[test]
fn usdc_is_token_zero_in_eight_decimals_on_both() {
    for network in Network::ALL {
        let usdc = network.usdc();
        assert_eq!((usdc.name, usdc.index, usdc.wei_decimals), ("USDC", 0, 8), "{network}");
        assert_eq!(usdc.wire, format!("{}:{}", usdc.name, usdc.token_id), "{network}");
        assert_eq!(usdc.token_id.len(), 34, "{network}");
    }
}

#[test]
fn a_user_signed_action_is_signed_under_its_network_s_chain() {
    for network in Network::ALL {
        let domain = Domain::user_signed(network);
        assert_eq!(domain.chain_id, network.signature_chain_id(), "{network}");
        assert_eq!((domain.name, domain.version), ("HyperliquidSignTransaction", "1"));
    }
}

#[test]
fn an_l1_action_is_signed_under_1337_on_both() {
    assert_eq!(Domain::EXCHANGE.chain_id, 1337);
}

#[test]
fn a_network_reads_and_writes_its_name() {
    assert_eq!("mainnet".parse::<Network>(), Ok(Network::Mainnet));
    assert_eq!("Testnet".parse::<Network>(), Ok(Network::Testnet));
    assert_eq!("devnet".parse::<Network>(), Err(UnknownNetwork("devnet".to_owned())));
    assert_eq!(Network::Mainnet.to_string(), "mainnet");
    assert_eq!(Network::Testnet.to_string(), "testnet");
}

#[test]
fn mainnet_is_the_default() {
    assert_eq!(Network::default(), Network::Mainnet);
}

#[test]
fn each_network_has_its_socket_and_its_hyperevm() {
    assert_eq!(Network::Mainnet.ws_url(), "wss://api.hyperliquid.xyz/ws");
    assert_eq!(Network::Testnet.ws_url(), "wss://api.hyperliquid-testnet.xyz/ws");
    assert_eq!(Network::Mainnet.evm_rpc_url(), "https://rpc.hyperliquid.xyz/evm");
    assert_eq!(Network::Testnet.evm_rpc_url(), "https://rpc.hyperliquid-testnet.xyz/evm");
    assert_eq!(Network::Mainnet.evm_chain_id(), 999);
    assert_eq!(Network::Testnet.evm_chain_id(), 998);
}

/// Both as a recorded `spotMeta` (2026-10-08) lists them. Testnet's token 150
/// is a different token.
#[test]
fn hype_is_token_150_on_mainnet_and_1105_on_testnet() {
    let mainnet = Network::Mainnet.hype();
    assert_eq!((mainnet.name, mainnet.index, mainnet.wei_decimals), ("HYPE", 150, 8));
    assert_eq!(mainnet.token_id, "0x0d01dc56dcaaca66ad901c959b4011ec");
    assert_eq!(mainnet.wire, "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec");
    let testnet = Network::Testnet.hype();
    assert_eq!((testnet.name, testnet.index, testnet.wei_decimals), ("HYPE", 1105, 8));
    assert_eq!(testnet.token_id, "0x7317beb7cceed72ef0b346074cc8e7ab");
    assert_eq!(testnet.wire, "HYPE:0x7317beb7cceed72ef0b346074cc8e7ab");
    for network in Network::ALL {
        let hype = network.hype();
        assert_eq!(hype.wire, format!("{}:{}", hype.name, hype.token_id), "{network}");
    }
}

/// The chain the bridge is on: a deposit's, not a signature's, though the
/// two agree today.
#[test]
fn the_bridge_is_on_arbitrum_one_and_arbitrum_sepolia() {
    assert_eq!(Network::Mainnet.arbitrum_chain_id(), 42_161);
    assert_eq!(Network::Testnet.arbitrum_chain_id(), 421_614);
}
