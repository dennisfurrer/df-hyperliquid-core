//! System addresses against the docs' examples (`hypercore-less-than-greater-
//! than-hyperevm-transfers`: token 200, HYPE, PURR) and hip4-tui's (token
//! 1452, KNTQ's 124).

use super::*;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn system_addresses_are_the_hype_one_and_every_token_s() {
    assert_eq!(system_address(200).to_hex(), "0x20000000000000000000000000000000000000c8");
    assert_eq!(system_address(1452).to_hex(), "0x20000000000000000000000000000000000005ac");
    assert_eq!(system_address(0).to_hex(), "0x2000000000000000000000000000000000000000");
    assert_eq!(HYPE_SYSTEM.to_hex(), "0x2222222222222222222222222222222222222222");
    for system in [
        HYPE_SYSTEM,
        system_address(0),
        system_address(150),
        system_address(u64::MAX),
        address("0x200000000000000000000000000000000000007c"),
        address("0x2000000000000000000000000000000000000001"),
    ] {
        assert!(is_system_address(system), "{system}");
    }
}

#[test]
fn a_person_is_never_taken_for_a_system_address() {
    for person in [
        address("0x0d1d9635d0640821d15e323ac8adadfa9c111414"),
        address("0x2000000000000000000000010000000000000001"),
        address("0x2100000000000000000000000000000000000001"),
        address("0x2222222222222222222222222222222222222223"),
        Address::ZERO,
    ] {
        assert!(!is_system_address(person), "{person}");
    }
}

#[test]
fn a_token_s_system_address_gives_back_its_index() {
    assert_eq!(system_token(system_address(1452)), Some(1452));
    assert_eq!(system_token(system_address(u64::MAX)), Some(u64::MAX));
    assert_eq!(system_token(HYPE_SYSTEM), None, "HYPE crosses natively, not as a token index");
    assert_eq!(system_token(Address::ZERO), None);
}
