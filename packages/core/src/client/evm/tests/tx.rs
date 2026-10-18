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
fn every_vector_is_rebuilt_byte_for_byte_and_a_creation_makes_its_address() {
    let cases = fixtures::cases();
    let viem = fixtures::viem();
    let ours = key(fixtures::text(&cases["key"]));
    assert_eq!(ours.address().to_hex(), "0x14791697260e4c9a71f18484c9f997b308e59325");
    let theirs = key(fixtures::text(&viem["key"]));
    for (file, key, count) in [(&cases, &ours, 3), (&viem, &theirs, 4)] {
        let transactions = file["transactions"].as_array().unwrap();
        assert_eq!(transactions.len(), count);
        for case in transactions {
            check(fixtures::text(&case["name"]), case, key);
        }
    }
    let creation = from_fields(&viem["transactions"][0]["fields"]);
    assert_eq!(creation.to, None, "a creation names no recipient");
    assert_eq!(creation.created(theirs.address()), Some(create_address(theirs.address(), 3)));
    assert_eq!(
        from_fields(&viem["transactions"][2]["fields"]).created(theirs.address()),
        None,
        "a call creates nothing"
    );
    for case in viem["created"].as_array().unwrap() {
        let from = fixtures::address(fixtures::text(&case["from"]));
        let nonce: u64 = fixtures::text(&case["nonce"]).parse().unwrap();
        assert_eq!(create_address(from, nonce).to_hex(), fixtures::text(&case["address"]), "nonce {nonce}");
    }
}

/// A live creation and a live CoreWriter send, as the node returns them.
#[test]
fn live_transactions_rebuilt_from_the_node_s_fields_hash_to_their_hash() {
    let receipt: Value = serde_json::from_str(&fixtures::creation(1)).unwrap();
    for answer in [fixtures::creation(0), fixtures::answer("eth_getTransactionByHash", 0)] {
        let tx: Value = serde_json::from_str(&answer).unwrap();
        let tx = &tx["result"];
        let rebuilt = from_fields(tx);
        let signature = Signature::from_parts(
            number(&tx["r"]).to_be_bytes(),
            number(&tx["s"]).to_be_bytes(),
            27 + u8::try_from(small(&tx["yParity"])).unwrap(),
        );
        assert_eq!(tx_hash(&rebuilt.signed(&signature).unwrap()).to_hex(), fixtures::text(&tx["hash"]));
        let from = fixtures::address(fixtures::text(&tx["from"]));
        assert_eq!(rebuilt.signer(&signature).unwrap(), from, "it recovers to the sender the node names");
        if let Some(created) = rebuilt.created(from) {
            assert_eq!(created.to_hex(), fixtures::text(&receipt["result"]["contractAddress"]));
            assert_eq!(created.to_hex(), fixtures::text(&fixtures::viem()["liveCreation"]["address"]));
        }
    }
}

#[test]
fn a_signature_without_a_recovery_bit_is_refused() {
    let tx = Eip1559 { chain_id: 999, gas: 21_000, ..Eip1559::default() };
    let odd = Signature::from_parts([1; 32], [1; 32], 29);
    assert_eq!(tx.signed(&odd), Err(crate::account::SignError::BadV(29)));
}

#[test]
fn a_call_becomes_the_transaction_viem_built_its_fees_and_gas_bounded_by_integers() {
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

    assert_eq!(Fees::over_base(U256::MAX, U256::ONE).max_fee, U256::MAX, "it saturates, never wraps");
    assert_eq!((with_headroom(45_000), with_headroom(u64::MAX)), (58_500, u64::MAX), "thirty percent over");
    let tx = Eip1559 { max_fee: U256::from(20_000_000u64), gas: 60_000, ..Eip1559::default() };
    assert_eq!(tx.gas_ceiling(), U256::from(1_200_000_000_000u64), "every unit of gas at the cap");
    let with_value = Eip1559 { value: U256::from(5u8), ..tx };
    assert_eq!(with_value.most_spent(), Some(U256::from(1_200_000_000_005u64)));
    assert_eq!(Eip1559 { value: U256::MAX, ..with_value }.most_spent(), None);
}
