//! System addresses against the docs' HyperCore and HyperEVM transfer examples (token 200, HYPE,
//! PURR) and recorded ones (token 1452, KNTQ's 124); linked tokens against the docs' examples (HYPE,
//! PURR's decimals); moves back to Core against viem's transactions and calldata.

use super::*;
use crate::client::evm::fixtures;
use crate::client::evm::tx::Fees;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

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
    // A recorded "HYPE back to Core" transaction, which viem 2.55.4 signed.
    let file = fixtures::cases();
    let case = &file["transactions"][1];
    let call = Linked::hype(8).to_core(dec("1.5")).unwrap();
    assert_eq!(call.to, Some(HYPE_SYSTEM));
    assert!(call.data.is_empty());
    let fees = Fees { max_priority_fee: U256::from(1_000_000_000u64), max_fee: U256::from(3_000_000_000u64) };
    let tx = call.transaction(999, 0, 21_000, fees);
    assert_eq!(crate::utils::hex::encode_prefixed(&tx.unsigned()), fixtures::text(&case["unsigned"]));
}

#[test]
fn a_token_goes_home_as_a_transfer_to_its_system_address() {
    // viem's transfer of 1.5e18 to token 150's system address.
    let viem = fixtures::viem();
    let contract = fixtures::address("0x5555555555555555555555555555555555555555");
    let token = Linked::erc20(150, contract, 8, 10);
    let call = token.to_core(dec("1.5")).unwrap();
    assert_eq!((call.to, call.value), (Some(contract), U256::ZERO));
    assert_eq!(crate::utils::hex::encode_prefixed(&call.data), fixtures::text(&viem["erc20"][0]["calldata"]));
    assert!(Linked::erc20(150, contract, 8, -2).to_core(dec("0.0000001")).is_err(), "finer than six places");
    assert!(token.to_core(dec("-1")).is_err());
    assert!(token.to_core(Dec::ZERO).is_err(), "nothing to move");
}

/// `0x2222...2222` for HYPE, `0x20`, zeros and the index for every other token; a person's
/// address, however near, is none of them.
#[test]
fn a_system_address_is_hype_s_or_0x20_and_a_token_index_and_nothing_else() {
    for (index, hex) in [
        (200, "0x20000000000000000000000000000000000000c8"),
        (1452, "0x20000000000000000000000000000000000005ac"),
        (124, "0x200000000000000000000000000000000000007c"),
        (0, "0x2000000000000000000000000000000000000000"),
    ] {
        assert_eq!(system_address(index).to_hex(), hex);
        assert_eq!(system_token(address(hex)), Some(index), "{hex}");
    }
    assert_eq!(HYPE_SYSTEM.to_hex(), "0x2222222222222222222222222222222222222222");
    for system in [
        HYPE_SYSTEM,
        system_address(150),
        system_address(u64::MAX),
        address("0x2000000000000000000000000000000000000001"),
    ] {
        assert!(is_system_address(system), "{system}");
    }
    assert_eq!(system_token(system_address(u64::MAX)), Some(u64::MAX));
    assert_eq!(system_token(HYPE_SYSTEM), None, "HYPE crosses natively, not as a token index");
    for person in [
        address("0x0d1d9635d0640821d15e323ac8adadfa9c111414"),
        address("0x2000000000000000000000010000000000000001"),
        address("0x2100000000000000000000000000000000000001"),
        address("0x2222222222222222222222222222222222222223"),
        Address::ZERO,
    ] {
        assert!(!is_system_address(person) && system_token(person).is_none(), "{person}");
    }
}
