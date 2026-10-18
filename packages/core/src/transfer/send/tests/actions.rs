//! The transfer actions as a wallet signs them: the EIP-712 JSON it is handed, and its 65-byte
//! answer checked against the account before anything is posted; and `sendAsset`'s forms. The
//! `@nktkas/hyperliquid` 0.33.2 vectors, byte for byte on both networks, are in
//! `tests/conformance/transfer.rs`.

use crate::account::{LocalKey, Signature, verify};
use crate::client::Network;
use crate::utils::sign::digest_of_wallet_json;
use crate::utils::{Dec, hex};
use serde_json::Value;

use super::*;
use crate::transfer::evm::HYPE_SYSTEM;
use crate::transfer::send::amount::Amount;

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const OWN: &str = "0x14791697260E4c9A71f18484C9f997B308e59325";
const FRIEND: &str = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414";
const HYPE: &str = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";
const NONCE: u64 = 1_790_412_105_001;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn amount(typed: &str) -> Amount {
    Amount::parse(typed).unwrap()
}

fn every_transfer() -> Vec<Box<dyn UserAction>> {
    vec![
        Box::new(UsdClassTransfer::new(amount("12.50"), true, NONCE)),
        Box::new(UsdSend::new(address(FRIEND), amount("7"), NONCE)),
        Box::new(SpotSend::new(address(FRIEND), HYPE, amount("0.5"), NONCE)),
        Box::new(SendAsset::new(HYPE_SYSTEM, SPOT_DEX, SPOT_DEX, HYPE, amount("1.25"), NONCE)),
        Box::new(Withdraw3::new(address(OWN), amount("25"), NONCE)),
    ]
}

/// The JSON a wallet is handed hashes, read back on its own, to the digest that is signed, and
/// names the posted action's fields in the posted order, with the same values.
#[test]
fn every_transfer_s_wallet_json_is_the_digest_and_the_posted_action() {
    for network in Network::ALL {
        for action in every_transfer() {
            let wallet = action.wallet_json(network);
            assert_eq!(digest_of_wallet_json(&wallet), Ok(action.digest(network)), "{wallet}");
            let typed: Value = serde_json::from_str(&wallet).unwrap();
            let posted: Value = serde_json::from_str(&action.wire(network)).unwrap();
            let primary = typed["primaryType"].as_str().unwrap();
            assert_eq!(primary, action.typed(network).primary_type());
            assert_eq!(typed["domain"]["chainId"], network.signature_chain_id());
            assert_eq!(typed["domain"]["name"], "HyperliquidSignTransaction");
            let names: Vec<&str> =
                typed["types"][primary].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
            let wire = action.action(network);
            let keys: Vec<&str> = wire.keys().filter(|key| !matches!(*key, "type" | "signatureChainId")).collect();
            assert_eq!(names, keys, "{network} {primary}: the typed fields are the posted keys");
            for name in names {
                assert_eq!(typed["message"][name], posted[name], "{network} {primary}: {name}");
            }
        }
    }
}

/// The browser's path, with a local key standing in for the wallet: it is handed the JSON, signs
/// what the JSON hashes to, and answers 65 bytes of hex, `v` as 0 or 1.
#[test]
fn a_wallet_s_answer_is_read_and_checked_against_the_account_before_it_is_posted() {
    let key = LocalKey::from_hex(KEY).unwrap();
    let send = SendAsset::usdc(Network::Mainnet, address(FRIEND), SPOT_DEX, SPOT_DEX, amount("25"), NONCE);
    let signed = key.sign_digest(&digest_of_wallet_json(&send.wallet_json(Network::Mainnet)).unwrap()).unwrap();
    let mut answer = [0u8; 65];
    answer[..64].copy_from_slice(&signed.to_compact());
    answer[64] = signed.v() - 27;
    let signature = Signature::from_hex(&hex::encode_prefixed(&answer)).unwrap();
    assert_eq!(verify(&send.digest(Network::Mainnet), &signature, address(OWN)), Ok(()));
    assert!(
        verify(&send.digest(Network::Mainnet), &signature, address(FRIEND)).is_err(),
        "a wallet signing as another account"
    );
    assert!(send.body(&signature, Network::Mainnet).starts_with(r#"{"action":{"type":"sendAsset","#));
}

/// USDC by each network's own token id; to oneself between two balances; from a subaccount, which
/// changes what is signed.
#[test]
fn send_asset_names_usdc_by_the_network_s_id_and_a_subaccount_when_sent_from_one() {
    for network in Network::ALL {
        let send = SendAsset::usdc(network, address(FRIEND), SPOT_DEX, SPOT_DEX, amount("25"), NONCE);
        assert!(send.wire(network).contains(&format!(r#""token":"{}""#, network.usdc().wire)), "{network}");
    }
    let cut = Amount::cut(Dec::parse("12.3456789").unwrap(), 6).unwrap();
    let combine = SendAsset::usdc(Network::Mainnet, address(OWN), PERP_DEX, SPOT_DEX, cut, NONCE);
    assert!(combine.wire(Network::Mainnet).contains(concat!(
        r#""destination":"0x14791697260e4c9a71f18484c9f997b308e59325","sourceDex":"","destinationDex":"spot","#,
        r#""token":"USDC:0x6d1e7cde53ba9467b783cb7c530ce054","amount":"12.345678","fromSubAccount":"""#
    )));
    let own = SendAsset::usdc(Network::Mainnet, address(OWN), SPOT_DEX, PERP_DEX, amount("1"), NONCE);
    let from_sub = own.clone().from_sub_account(address(FRIEND));
    assert!(
        from_sub.wire(Network::Mainnet).contains(r#""fromSubAccount":"0x0d1d9635d0640821d15e323ac8adadfa9c111414""#)
    );
    assert_ne!(from_sub.digest(Network::Mainnet), own.digest(Network::Mainnet));
}
