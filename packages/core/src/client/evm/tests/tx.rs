//! Transactions against viem (`viem_evm.json`: serializeTransaction and
//! signTransaction, getContractAddress), against vectors made by viem 2.55.4
//! and 2.50.4, and against HyperEVM mainnet itself: a live contract creation
//! and a live call rebuilt from the node's own fields hash to the node's own
//! transaction hash.

use crate::account::{LocalKey, Signature};
use serde_json::Value;

use super::*;
use crate::client::evm::fixtures;

fn number(value: &Value) -> U256 {
    let text = fixtures::text(value);
    text.parse().unwrap()
}

fn small(value: &Value) -> u64 {
    number(value).to_u64().unwrap()
}

/// A transaction from the fields a vector or a node writes: decimal or hex numbers.
fn from_fields(fields: &Value) -> Eip1559 {
    let data = fields.get("data").or_else(|| fields.get("input")).unwrap();
    let access_list = fields
        .get("accessList")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| AccessItem {
                    address: fixtures::address(fixtures::text(&item["address"])),
                    storage_keys: item["storageKeys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| fixtures::text(key).parse().unwrap())
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    Eip1559 {
        chain_id: small(&fields["chainId"]),
        nonce: small(&fields["nonce"]),
        max_priority_fee: number(&fields["maxPriorityFeePerGas"]),
        max_fee: number(&fields["maxFeePerGas"]),
        gas: small(&fields["gas"]),
        to: fields.get("to").and_then(Value::as_str).map(fixtures::address),
        value: number(&fields["value"]),
        data: fixtures::bytes(data),
        access_list,
    }
}

fn key(text: &str) -> LocalKey {
    LocalKey::from_hex(text).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    crate::utils::hex::encode_prefixed(bytes)
}

fn check(name: &str, case: &Value, key: &LocalKey) {
    let tx = from_fields(&case["fields"]);
    assert_eq!(hex(&tx.unsigned()), fixtures::text(&case["unsigned"]), "{name}: unsigned");
    assert_eq!(tx.signing_hash().to_hex(), fixtures::text(&case["signingHash"]), "{name}: signing hash");
    let signed = tx.sign(key).unwrap();
    assert_eq!(hex(&signed), fixtures::text(&case["signed"]), "{name}: signed");
    if let Some(hash) = case.get("hash") {
        assert_eq!(tx_hash(&signed).to_hex(), fixtures::text(hash), "{name}: hash");
    }
    // The same bytes from a signature made elsewhere, and it recovers to the key.
    let signature = key.sign_digest(&tx.signing_hash().0).unwrap();
    assert_eq!(tx.signed(&signature).unwrap(), signed, "{name}: signed from a signature");
    assert_eq!(tx.signer(&signature).unwrap(), key.address(), "{name}: signer");
}

#[test]
fn recorded_transactions_are_rebuilt_byte_for_byte() {
    let file = fixtures::cases();
    let key = key(fixtures::text(&file["key"]));
    assert_eq!(key.address().to_hex(), "0x14791697260e4c9a71f18484c9f997b308e59325");
    let cases = file["transactions"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        check(fixtures::text(&case["name"]), case, &key);
    }
}

#[test]
fn creations_access_lists_and_wide_values_match_viem() {
    let viem = fixtures::viem();
    let key = key(fixtures::text(&viem["key"]));
    let cases = viem["transactions"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        check(fixtures::text(&case["name"]), case, &key);
    }
    let creation = from_fields(&cases[0]["fields"]);
    assert_eq!(creation.to, None, "a creation names no recipient");
    assert_eq!(creation.created(key.address()), Some(create_address(key.address(), 3)));
    assert_eq!(from_fields(&cases[2]["fields"]).created(key.address()), None, "a call creates nothing");
}

#[test]
fn a_live_creation_hashes_to_its_hash_and_creates_the_address_the_node_reports() {
    let tx: Value = serde_json::from_str(&fixtures::creation(0)).unwrap();
    let receipt: Value = serde_json::from_str(&fixtures::creation(1)).unwrap();
    let (tx, receipt) = (&tx["result"], &receipt["result"]);
    assert_eq!(tx["to"], Value::Null);
    let rebuilt = from_fields(tx);
    let signature = Signature::from_parts(
        number(&tx["r"]).to_be_bytes(),
        number(&tx["s"]).to_be_bytes(),
        27 + u8::try_from(small(&tx["yParity"])).unwrap(),
    );
    let raw = rebuilt.signed(&signature).unwrap();
    assert_eq!(tx_hash(&raw).to_hex(), fixtures::text(&tx["hash"]));
    let from = fixtures::address(fixtures::text(&tx["from"]));
    assert_eq!(rebuilt.signer(&signature).unwrap(), from, "it recovers to the sender the node names");
    let created = rebuilt.created(from).unwrap();
    assert_eq!(created.to_hex(), fixtures::text(&receipt["contractAddress"]));
    assert_eq!(created.to_hex(), fixtures::text(&fixtures::viem()["liveCreation"]["address"]));
}

#[test]
fn a_live_call_hashes_to_its_hash() {
    // A CoreWriter send on mainnet, as the node returns it.
    let tx: Value = serde_json::from_str(&fixtures::answer("eth_getTransactionByHash", 0)).unwrap();
    let tx = &tx["result"];
    let rebuilt = from_fields(tx);
    let signature = Signature::from_parts(
        number(&tx["r"]).to_be_bytes(),
        number(&tx["s"]).to_be_bytes(),
        27 + u8::try_from(small(&tx["yParity"])).unwrap(),
    );
    assert_eq!(tx_hash(&rebuilt.signed(&signature).unwrap()).to_hex(), fixtures::text(&tx["hash"]));
    assert_eq!(rebuilt.signer(&signature).unwrap().to_hex(), fixtures::text(&tx["from"]));
}

#[test]
fn create_addresses_match_viem_for_every_nonce_width() {
    let viem = fixtures::viem();
    for case in viem["created"].as_array().unwrap() {
        let from = fixtures::address(fixtures::text(&case["from"]));
        let nonce: u64 = fixtures::text(&case["nonce"]).parse().unwrap();
        assert_eq!(create_address(from, nonce).to_hex(), fixtures::text(&case["address"]), "nonce {nonce}");
    }
}

#[test]
fn a_signature_without_a_recovery_bit_is_refused() {
    let tx = Eip1559 { chain_id: 999, gas: 21_000, ..Eip1559::default() };
    let odd = Signature::from_parts([1; 32], [1; 32], 29);
    assert_eq!(tx.signed(&odd), Err(crate::account::SignError::BadV(29)));
}

#[test]
fn a_call_becomes_the_transaction_viem_built_for_it() {
    // A CoreWriter order on testnet: base fee 100000000, tip 1000000.
    let file = fixtures::cases();
    let case = &file["transactions"][2];
    let call = Call {
        to: Some(fixtures::address("0x3333333333333333333333333333333333333333")),
        value: U256::ZERO,
        data: fixtures::bytes(&case["fields"]["data"]),
    };
    let fees = Fees::over_base(U256::from(100_000_000u64), U256::from(1_000_000u64));
    assert_eq!(fees.max_fee, U256::from(201_000_000u64), "twice the base fee and the tip");
    let tx = call.transaction(998, 12, 58_500, fees);
    assert_eq!(hex(&tx.unsigned()), fixtures::text(&case["unsigned"]));
}

#[test]
fn fees_and_gas_are_bounded_by_integers() {
    assert_eq!(Fees::over_base(U256::from(20_000_000u64), U256::ZERO).max_fee, U256::from(40_000_000u64));
    assert_eq!(Fees::over_base(U256::MAX, U256::ONE).max_fee, U256::MAX, "it saturates, never wraps");
    assert_eq!(with_headroom(45_000), 58_500, "thirty percent over the estimate");
    assert_eq!(with_headroom(u64::MAX), u64::MAX);
    let tx = Eip1559 { max_fee: U256::from(20_000_000u64), gas: 60_000, ..Eip1559::default() };
    assert_eq!(tx.gas_ceiling(), U256::from(1_200_000_000_000u64), "every unit of gas at the cap");
    let with_value = Eip1559 { value: U256::from(5u8), ..tx };
    assert_eq!(with_value.most_spent(), Some(U256::from(1_200_000_000_005u64)));
    assert_eq!(Eip1559 { value: U256::MAX, ..with_value }.most_spent(), None);
}
