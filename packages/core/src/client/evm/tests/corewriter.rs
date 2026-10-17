//! Payloads and calldata against viem (every action id, `viem_evm.json`); the
//! decoder against live `RawAction` logs off `0x3333...3333` on mainnet
//! (windows captured 2026-09-26 and 2026-10-08, and a send's receipt), each
//! read by viem's `decodeAbiParameters` as well.

use crate::client::Network;
use crate::utils::Dec;
use serde_json::Value;

use super::*;
use crate::client::evm::fixtures;
use crate::client::evm::records::Log;
use crate::client::evm::rpc::eth;
use crate::client::evm::tx::Fees;
use crate::client::evm::u256::{B256, U256};

fn n(value: &Value) -> u64 {
    fixtures::text(value).parse().unwrap()
}

fn n128(value: &Value) -> u128 {
    fixtures::text(value).parse().unwrap()
}

fn byte(value: &Value) -> u8 {
    u8::try_from(n(value)).unwrap()
}

fn small(value: &Value) -> u32 {
    u32::try_from(n(value)).unwrap()
}

fn flag(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        other => fixtures::text(other) == "true",
    }
}

fn who(value: &Value) -> Address {
    fixtures::address(fixtures::text(value))
}

/// An action from the docs' fields in the docs' order, as a vector lists them.
fn action(id: u64, v: &[Value]) -> Action {
    match id {
        1 => Action::LimitOrder {
            asset: small(&v[0]),
            is_buy: flag(&v[1]),
            limit_px: n(&v[2]),
            sz: n(&v[3]),
            reduce_only: flag(&v[4]),
            tif: Tif::from_code(byte(&v[5])),
            cloid: n128(&v[6]),
        },
        2 => Action::VaultTransfer { vault: who(&v[0]), is_deposit: flag(&v[1]), usd: n(&v[2]) },
        3 => Action::TokenDelegate { validator: who(&v[0]), wei: n(&v[1]), is_undelegate: flag(&v[2]) },
        4 => Action::StakingDeposit { wei: n(&v[0]) },
        5 => Action::StakingWithdraw { wei: n(&v[0]) },
        6 => Action::SpotSend { destination: who(&v[0]), token: n(&v[1]), wei: n(&v[2]) },
        7 => Action::UsdClassTransfer { ntl: n(&v[0]), to_perp: flag(&v[1]) },
        8 => Action::FinalizeEvmContract {
            token: n(&v[0]),
            variant: FinalizeVariant::from_code(byte(&v[1])),
            create_nonce: n(&v[2]),
        },
        9 => Action::AddApiWallet { wallet: who(&v[0]), name: fixtures::text(&v[1]).to_owned() },
        10 => Action::CancelByOid { asset: small(&v[0]), oid: n(&v[1]) },
        11 => Action::CancelByCloid { asset: small(&v[0]), cloid: n128(&v[1]) },
        12 => Action::ApproveBuilderFee { max_fee_rate: n(&v[0]), builder: who(&v[1]) },
        13 => Action::SendAsset {
            destination: who(&v[0]),
            sub_account: who(&v[1]),
            source_dex: small(&v[2]),
            destination_dex: small(&v[3]),
            token: n(&v[4]),
            wei: n(&v[5]),
        },
        15 => Action::BorrowLend { operation: BorrowLendOp::from_code(byte(&v[0])), token: n(&v[1]), wei: n(&v[2]) },
        16 => Action::SetAbstraction { user: who(&v[0]), abstraction: Abstraction::from_code(byte(&v[1])) },
        17 => Action::Outcome {
            operation: OutcomeOp::from_code(byte(&v[0])),
            question: small(&v[1]),
            outcome: small(&v[2]),
            wei: n(&v[3]),
        },
        other => panic!("no action {other}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    crate::utils::hex::encode_prefixed(bytes)
}

/// A log as a vector keeps it. Some were filtered on CoreWriter alone and
/// kept without their `address`, so it is put back.
fn log(value: &Value) -> Log {
    let mut value = value.clone();
    if value.get("address").is_none() {
        value["address"] = Value::String(CORE_WRITER.to_hex());
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn the_contract_selector_and_topic_are_the_hashes_they_claim_to_be() {
    assert_eq!(CORE_WRITER.to_hex(), "0x3333333333333333333333333333333333333333");
    assert_eq!(
        RAW_ACTION_TOPIC,
        crate::client::evm::abi::event_topic("event RawAction(address indexed user, bytes data)").unwrap()
    );
    assert_eq!(RAW_ACTION_TOPIC.to_hex(), "0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3");
    assert_eq!(SEND_RAW_ACTION, crate::client::evm::abi::selector("sendRawAction(bytes)").unwrap());
    assert_eq!(hex(&SEND_RAW_ACTION), "0x17938e13");
    let ids: Vec<u32> = SPECS.iter().map(|spec| spec.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17], "the docs' table skips 14");
    for spec in &SPECS {
        let types = crate::client::evm::abi::params(spec.types).unwrap();
        assert_eq!(types.len(), spec.fields.len(), "{}", spec.name);
    }
}

#[test]
fn every_action_id_encodes_as_viem_encodes_it() {
    let viem = fixtures::viem();
    let cases = viem["corewriter"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    for case in cases {
        let id = case["id"].as_u64().unwrap();
        let built = action(id, case["values"].as_array().unwrap());
        assert_eq!(u64::from(built.id()), id);
        assert_eq!(built.spec().types, fixtures::text(&case["types"]), "{id}: the docs' types");
        assert_eq!(hex(&built.payload()), fixtures::text(&case["payload"]), "{id}: payload");
        assert_eq!(hex(&built.calldata()), fixtures::text(&case["calldata"]), "{id}: calldata");
        assert_eq!(Action::decode(&built.payload()), Ok(built.clone()), "{id}: read back");
    }
}

#[test]
fn hip4_tuis_payloads_and_calldata_are_rebuilt() {
    let hip4 = fixtures::hip4_tui();
    for case in hip4["corewriter"]["encoded"].as_array().unwrap() {
        let payload = fixtures::bytes(&case["payload"]);
        let read = Action::decode(&payload).unwrap();
        assert_eq!(u64::from(read.id()), case["id"].as_u64().unwrap());
        assert_eq!(read.payload(), payload);
        if let Some(values) = case.get("values") {
            assert_eq!(action(case["id"].as_u64().unwrap(), values.as_array().unwrap()), read);
        }
        if let Some(calldata) = case.get("calldata") {
            assert_eq!(hex(&read.calldata()), fixtures::text(calldata));
        }
    }
}

#[test]
fn hip4_tuis_live_logs_decode_and_round_trip() {
    let hip4 = fixtures::hip4_tui();
    let logs = hip4["corewriter"]["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 8);
    for raw in logs {
        let read = RawAction::from_log(&log(raw)).unwrap();
        let action = read.action.clone().unwrap();
        assert_eq!(action.payload(), read.payload, "the encoder gives back the logged bytes");
    }
    // The first: USDC's system contract sending asset, as viem read it.
    let first = RawAction::from_log(&log(&logs[0])).unwrap();
    assert_eq!(first.user.to_hex(), "0x6b9e773128f453f5c2c60935ee2de2cbc5390a24");
    assert_eq!(
        first.action,
        Ok(Action::SendAsset {
            destination: fixtures::address("0xc0a6a8da035f8010bdc280e16f4bc2af415bb0c3"),
            sub_account: Address::ZERO,
            source_dex: SPOT_DEX,
            destination_dex: 0,
            token: 0,
            wei: 3_814_000_000,
        })
    );
    // The seventh: a staking deposit.
    assert_eq!(
        RawAction::from_log(&log(&logs[6])).unwrap().action,
        Ok(Action::StakingDeposit { wei: 1_000_249_648_235 })
    );
}

#[test]
fn a_live_window_of_logs_decodes_as_viem_reads_it() {
    let answer = fixtures::answer("eth_getLogs", 1);
    let logs = eth::logs(&raw_action_logs(
        crate::client::evm::rpc::BlockTag::Latest,
        crate::client::evm::rpc::BlockTag::Latest,
    ))
    .answer(&answer)
    .unwrap();
    let viem = fixtures::viem();
    let expected = viem["liveLogs"].as_array().unwrap();
    assert_eq!(logs.len(), expected.len());
    for (one, said) in logs.iter().zip(expected) {
        let read = RawAction::from_log(one).unwrap();
        assert_eq!(read.user.to_hex(), fixtures::text(&said["user"]));
        assert_eq!(read.version, 1);
        assert_eq!(read.action, Ok(action(said["id"].as_u64().unwrap(), said["values"].as_array().unwrap())));
    }
}

#[test]
fn a_live_sends_receipt_carries_its_action() {
    let receipt = eth::receipt(B256::ZERO).answer(&fixtures::answer("eth_getTransactionReceipt", 0)).unwrap().unwrap();
    let actions: Vec<RawAction> = receipt.logs.iter().filter_map(|one| RawAction::from_log(one).ok()).collect();
    assert_eq!(actions.len(), 1, "one RawAction among the USDC logs");
    let viem = fixtures::viem();
    assert_eq!(hex(&actions[0].payload), fixtures::text(&viem["rawAction"]["payload"]));
    assert!(matches!(actions[0].action, Ok(Action::SendAsset { .. })));
    let usdc_log = receipt.logs.iter().find(|one| one.address != CORE_WRITER).unwrap();
    assert_eq!(RawAction::from_log(usdc_log), Err(BadAction::NotRawAction), "another contract's log");
}

#[test]
fn what_the_table_does_not_have_is_called_out() {
    assert_eq!(Action::decode(&[1, 0, 0, 14]), Err(BadAction::Gap));
    assert_eq!(Action::decode(&[1, 0, 0, 99]), Err(BadAction::UnknownId(99)));
    assert_eq!(Action::decode(&[1, 0]), Err(BadAction::Short(2)));
    assert_eq!(Action::decode(&[2, 0, 0, 4]), Err(BadAction::Version(2)));
    let deposit = Action::StakingDeposit { wei: 5 }.payload();
    assert!(matches!(Action::decode(&deposit[..20]), Err(BadAction::Abi(_))));
    // An enum value the docs do not name still decodes, and encodes back the same.
    let odd = Action::LimitOrder {
        asset: 0,
        is_buy: true,
        limit_px: 1,
        sz: 1,
        reduce_only: false,
        tif: Tif::Other(9),
        cloid: 0,
    };
    assert_eq!(Action::decode(&odd.payload()), Ok(odd));
}

#[test]
fn a_send_is_a_valueless_transaction_to_corewriter() {
    // A CoreWriter order on testnet, which viem 2.50.4 built and signed.
    let hip4 = fixtures::hip4_tui();
    let case = &hip4["transactions"][2];
    let order = Action::decode(&fixtures::bytes(&hip4["corewriter"]["encoded"][3]["payload"])).unwrap();
    let call = order.call();
    assert_eq!((call.to, call.value), (Some(CORE_WRITER), U256::ZERO), "value voids an action");
    let fees = Fees::over_base(U256::from(100_000_000u64), U256::from(1_000_000u64));
    let tx = order.transaction(Network::Testnet, 12, 58_500, fees);
    assert_eq!(tx.chain_id, 998);
    assert_eq!(hex(&tx.unsigned()), fixtures::text(&case["unsigned"]));
    assert_eq!(order.transaction(Network::Mainnet, 0, 1, fees).chain_id, 999);
}

#[test]
fn amounts_are_scaled_by_their_units_and_never_rounded() {
    let dec = |text: &str| Dec::parse(text).unwrap();
    // A live limit order: sell 7 HYPE at 1.52, both at 1e8.
    let order = Action::limit_order(25, false, dec("1.52"), dec("7"), true, Tif::Ioc, 0).unwrap();
    assert!(matches!(order, Action::LimitOrder { limit_px: 152_000_000, sz: 700_000_000, .. }));
    assert!(Action::limit_order(25, false, dec("1.000000001"), dec("7"), true, Tif::Ioc, 0).is_err(), "nine places");
    assert!(Action::limit_order(25, false, Dec::ZERO, dec("7"), true, Tif::Ioc, 0).is_err());
    assert_eq!(
        Action::vault_transfer(Address::ZERO, true, dec("12.5")),
        Ok(Action::VaultTransfer { vault: Address::ZERO, is_deposit: true, usd: 12_500_000 })
    );
    assert_eq!(
        Action::usd_class_transfer(dec("11.318565"), false),
        Ok(Action::UsdClassTransfer { ntl: 11_318_565, to_perp: false })
    );
    assert_eq!(Action::staking_deposit(dec("10002.49648235")), Ok(Action::StakingDeposit { wei: 1_000_249_648_235 }));
    assert_eq!(Action::staking_withdraw(dec("1.5")), Ok(Action::StakingWithdraw { wei: 150_000_000 }));
    assert_eq!(
        Action::token_delegate(Address::ZERO, dec("25"), false),
        Ok(Action::TokenDelegate { validator: Address::ZERO, wei: 2_500_000_000, is_undelegate: false })
    );
    assert!(Action::usd_class_transfer(dec("0.0000001"), true).is_err());
}

#[test]
fn enum_codes_read_both_ways() {
    for (tif, code) in [(Tif::Alo, 1), (Tif::Gtc, 2), (Tif::Ioc, 3), (Tif::Other(0), 0)] {
        assert_eq!((tif.code(), Tif::from_code(code)), (code, tif));
    }
    for (op, code) in [
        (BorrowLendOp::Supply, 0),
        (BorrowLendOp::Withdraw, 1),
        (BorrowLendOp::Borrow, 2),
        (BorrowLendOp::Repay, 3),
        (BorrowLendOp::Other(4), 4),
    ] {
        assert_eq!((op.code(), BorrowLendOp::from_code(code)), (code, op));
    }
    for (variant, code) in
        [(FinalizeVariant::Create, 1), (FinalizeVariant::FirstStorageSlot, 2), (FinalizeVariant::CustomStorageSlot, 3)]
    {
        assert_eq!((variant.code(), FinalizeVariant::from_code(code)), (code, variant));
    }
    for (mode, code) in
        [(Abstraction::Disabled, 1), (Abstraction::UnifiedAccount, 2), (Abstraction::PortfolioMargin, 3)]
    {
        assert_eq!((mode.code(), Abstraction::from_code(code)), (code, mode));
    }
    for (op, code) in [
        (OutcomeOp::SplitOutcome, 0),
        (OutcomeOp::MergeOutcome, 1),
        (OutcomeOp::MergeQuestion, 2),
        (OutcomeOp::NegateOutcome, 3),
    ] {
        assert_eq!((op.code(), OutcomeOp::from_code(code)), (code, op));
    }
}
