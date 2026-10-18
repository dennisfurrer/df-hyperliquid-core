//! Payloads and calldata against viem, every action id (`viem_evm.json`), and a
//! CoreWriter order on testnet that viem built and signed.

use crate::client::Network;
use crate::client::evm::corewriter::support::{action, hex};
use crate::client::evm::fixtures;
use crate::utils::Dec;

use super::*;

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
fn a_send_is_a_valueless_transaction_to_corewriter() {
    // A CoreWriter order on testnet, which viem 2.50.4 built and signed.
    let file = fixtures::cases();
    let case = &file["transactions"][2];
    let order = Action::decode(&fixtures::bytes(&file["corewriter"]["encoded"][3]["payload"])).unwrap();
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
