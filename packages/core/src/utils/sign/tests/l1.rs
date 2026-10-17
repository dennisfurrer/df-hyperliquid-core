//! L1 vectors. The first three were made by `@nktkas/hyperliquid` 0.33.2
//! itself (its own action functions, the key below, nonce 1790412105001, a
//! transport that recorded the body); the connection id is the SDK's
//! `createL1ActionHash`. The last is the official Python SDK's test
//! `test_l1_action_signing_matches_with_vault`.

use super::*;
use crate::account::{LocalKey, recover};
use crate::client::Network;
use crate::utils::hex;
use crate::utils::sign::eip712::digest_of_wallet_json;
use crate::utils::sign::keccak::keccak256;
use crate::utils::sign::msgpack::Value;

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_790_412_105_001;

fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

/// `vaultTransfer`, `usd` in millionths.
struct VaultTransfer {
    vault: Address,
    is_deposit: bool,
    usd: u64,
}

impl L1Action for VaultTransfer {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "vaultTransfer")
            .put("vaultAddress", self.vault.to_hex())
            .put("isDeposit", self.is_deposit)
            .put("usd", self.usd)
    }
}

/// `borrowLend`; an amount of `None` is everything, and goes as nil.
struct BorrowLend {
    operation: &'static str,
    token: u64,
    amount: Option<&'static str>,
}

impl L1Action for BorrowLend {
    fn action(&self) -> Map {
        Map::new().put("type", "borrowLend").put("operation", self.operation).put("token", self.token).put(
            "amount",
            match self.amount {
                Some(amount) => Value::from(amount),
                None => Value::Nil,
            },
        )
    }
}

fn hlp() -> VaultTransfer {
    VaultTransfer {
        vault: "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303".parse().unwrap(),
        is_deposit: true,
        usd: 10_000_000,
    }
}

/// `(action, posted json, connection id, r, s, v)`.
#[test]
fn every_l1_action_hashes_and_signs_as_the_sdk_does() {
    let key = key();
    let cases: [(&dyn L1Action, &str, &str, &str, &str, u8); 3] = [
        (
            &hlp(),
            r#"{"type":"vaultTransfer","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","isDeposit":true,"usd":10000000}"#,
            "0x58a49fb2e400641875c0df3c98fd48f61210198012623c24f1ce8cc748c6e57a",
            "0xd7b57bb7650ca10752a733ed62df2a5a8931e1c4f7290e0a143bcf96b22ad1fb",
            "0x63045ef3dfa4f14c26ac85dc03a76b4c332bbca837fb52ea4f9cf84bfcabe450",
            28,
        ),
        (
            &BorrowLend { operation: "supply", token: 0, amount: Some("100.5") },
            r#"{"type":"borrowLend","operation":"supply","token":0,"amount":"100.5"}"#,
            "0x1d8d7b38e50758c4781c5cc17e2bb64dcd31c7e1870293da248ebb3ad9743730",
            "0x165ca9fe188b157c2203f6d8e1ebd2204d7b939652d29c0626d0c4ca642e662f",
            "0x1e22b483499c2d5f0ccaf2773bfd22cde99cb372f3f437bbf125d636e294edfd",
            28,
        ),
        (
            &BorrowLend { operation: "repay", token: 150, amount: None },
            r#"{"type":"borrowLend","operation":"repay","token":150,"amount":null}"#,
            "0xa43161edaad13340450e7eb286453bbe1aca5ec7e278baefb171120a4a7e26f4",
            "0xf0e249f385d377f2748b79f88ac039feebd4c458012bacd8e09839ea5d725585",
            "0x7fec0e9e5ab28c4f79e28c2b2c27fd8d3f02db82de777fa1cd6bf1019b45830b",
            27,
        ),
    ];
    for (action, json, id, r, s, v) in cases {
        assert_eq!(action.wire(), json, "the posted action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(NONCE, None)), id, "{json}: the connection id");
        let digest = action.digest(NONCE, None, Network::Mainnet);
        assert_eq!(digest, l1_digest(&action.msgpack(), NONCE, None, Network::Mainnet));
        let signature = key.sign_digest(&digest).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(recover(&digest, &signature), Ok(key.address()));
        assert_eq!(
            digest_of_wallet_json(&action.wallet_json(NONCE, None, Network::Mainnet)),
            Ok(digest),
            "{json}: wallet json"
        );
    }
}

#[test]
fn an_l1_body_is_action_nonce_signature_and_a_null_vault() {
    let action = hlp();
    let signature = key().sign_digest(&action.digest(NONCE, None, Network::Mainnet)).unwrap();
    assert_eq!(
        action.body(NONCE, &signature, None),
        concat!(
            r#"{"action":{"type":"vaultTransfer","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","#,
            r#""isDeposit":true,"usd":10000000},"nonce":1790412105001,"#,
            r#""signature":{"r":"0xd7b57bb7650ca10752a733ed62df2a5a8931e1c4f7290e0a143bcf96b22ad1fb","#,
            r#""s":"0x63045ef3dfa4f14c26ac85dc03a76b4c332bbca837fb52ea4f9cf84bfcabe450","v":28},"#,
            r#""vaultAddress":null}"#,
        )
    );
}

/// The Python SDK writes `r` without its leading zero byte; the venue reads
/// either, and this build writes the full word as the TypeScript SDK does.
#[test]
fn acting_for_a_vault_matches_the_python_sdk_s_own_test() {
    let action = Map::new().put("type", "dummy").put("num", 100_000_000_000u64);
    assert_eq!(hex::encode_prefixed(&action.msgpack()), "0x82a474797065a564756d6d79a36e756dcf000000174876e800");
    let vault: Address = "0x1719884eb866cb12b2287399b15f7db5e7d775ea".parse().unwrap();
    assert_eq!(
        hex::encode_prefixed(&action.connection_id(0, Some(vault))),
        "0xde9e09a7a3da45cc694096d4bfdcd89bc1b892d05497c5ecc1f56c335945184c"
    );
    let signature = key().sign_digest(&action.digest(0, Some(vault), Network::Mainnet)).unwrap();
    assert_eq!(signature.r_hex(), "0x003c548db75e479f8012acf3000ca3a6b05606bc2ec0c29c50c515066a326239");
    assert_eq!(signature.s_hex(), "0x4d402be7396ce74fbba3795769cda45aec00dc3125a984f2a9f23177b190da2c");
    assert_eq!(signature.v(), 28);
    assert!(
        action
            .body(0, &signature, Some(vault))
            .ends_with(r#""vaultAddress":"0x1719884eb866cb12b2287399b15f7db5e7d775ea"}"#)
    );
}

#[test]
fn every_part_of_the_envelope_reaches_the_hash() {
    let packed = hlp().msgpack();
    let plain = connection_id(&packed, NONCE, None);
    let vault: Address = "0x1111111111111111111111111111111111111111".parse().unwrap();
    let in_vault = connection_id(&packed, NONCE, Some(vault));
    assert_ne!(plain, in_vault, "the vault marker was not hashed");
    assert_ne!(in_vault, connection_id(&packed, NONCE, Some(Address::ZERO)), "the vault address was not hashed");
    assert_ne!(plain, connection_id(&packed, NONCE + 1, None), "the nonce was not hashed");

    let mut preimage = packed.clone();
    preimage.extend_from_slice(&NONCE.to_be_bytes());
    preimage.push(0x01);
    preimage.extend_from_slice(vault.bytes());
    assert_eq!(in_vault, keccak256(&preimage));
    let mut bare = packed;
    bare.extend_from_slice(&NONCE.to_be_bytes());
    bare.push(0x00);
    assert_eq!(plain, keccak256(&bare));
}

#[test]
fn the_phantom_agent_names_its_network_as_a_or_b() {
    for (network, source) in [(Network::Mainnet, "a"), (Network::Testnet, "b")] {
        let agent = phantom_agent([9; 32], network);
        assert_eq!(agent.type_signature(), "Agent(string source,bytes32 connectionId)");
        assert_eq!(agent.fields()[0].1, Field::String(source), "{network}");
    }
}

#[test]
fn the_same_action_signs_apart_on_the_two_networks() {
    let action = hlp();
    let mainnet = action.digest(NONCE, None, Network::Mainnet);
    let testnet = action.digest(NONCE, None, Network::Testnet);
    assert_ne!(mainnet, testnet, "a testnet signature must never be a mainnet one");
    assert_eq!(action.connection_id(NONCE, None), action.connection_id(NONCE, None), "the hash itself has no network");
}
