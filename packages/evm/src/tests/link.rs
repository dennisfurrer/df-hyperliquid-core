//! Linked tokens against the docs' examples (`hypercore-less-than-greater-
//! than-hyperevm-transfers`: HYPE, PURR's decimals); moves back to Core against
//! viem's transactions and calldata. The system addresses themselves are
//! hl-core's, tested there.

use hl_core::Dec;

use super::*;
use crate::{fixtures, tx::Fees};

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn decimals_on_each_side_follow_the_link() {
    // The docs' PURR: five wei decimals on Core, thirteen more on the EVM.
    let purr = Linked::erc20(1, fixtures::address("0x9b498c3c8a0b8cd8ba1d9851d40d186f1872b44e"), 5, 13);
    assert_eq!(purr.evm_decimals(), Some(18));
    assert_eq!(purr.core_places(), Some(5));
    assert_eq!(purr.system(), system_address(1));
    let hype = Linked::hype(8);
    assert_eq!((hype.evm_decimals(), hype.core_places(), hype.system()), (Some(18), Some(8), HYPE_SYSTEM));
    let fewer = Linked::erc20(7, Address::ZERO, 8, -2);
    assert_eq!((fewer.evm_decimals(), fewer.core_places()), (Some(6), Some(6)), "the fewer of the two");
    assert_eq!(Linked::erc20(7, Address::ZERO, 1, -2).evm_decimals(), None);
}

#[test]
fn hype_goes_home_as_a_plain_value_transfer() {
    // hip4-tui's "HYPE back to Core" transaction, which viem 2.55.4 signed.
    let hip4 = fixtures::hip4_tui();
    let case = &hip4["transactions"][1];
    let call = Linked::hype(8).to_core(dec("1.5")).unwrap();
    assert_eq!(call.to, Some(HYPE_SYSTEM));
    assert!(call.data.is_empty());
    let fees = Fees { max_priority_fee: U256::from(1_000_000_000u64), max_fee: U256::from(3_000_000_000u64) };
    let tx = call.transaction(999, 0, 21_000, fees);
    assert_eq!(hl_core::hex::encode_prefixed(&tx.unsigned()), fixtures::text(&case["unsigned"]));
}

#[test]
fn a_token_goes_home_as_a_transfer_to_its_system_address() {
    // viem's transfer of 1.5e18 to token 150's system address.
    let viem = fixtures::viem();
    let contract = fixtures::address("0x5555555555555555555555555555555555555555");
    let token = Linked::erc20(150, contract, 8, 10);
    let call = token.to_core(dec("1.5")).unwrap();
    assert_eq!((call.to, call.value), (Some(contract), U256::ZERO));
    assert_eq!(hl_core::hex::encode_prefixed(&call.data), fixtures::text(&viem["erc20"][0]["calldata"]));
    assert!(Linked::erc20(150, contract, 8, -2).to_core(dec("0.0000001")).is_err(), "finer than six places");
    assert!(token.to_core(dec("-1")).is_err());
    assert!(token.to_core(Dec::ZERO).is_err(), "nothing to move");
}
