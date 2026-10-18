//! Expected values are node's `BigInt` over the same digits, and live
//! answers from `vectors/hyperevm_mainnet.json`.

use super::*;

const MAX_DECIMAL: &str = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

fn big(text: &str) -> U256 {
    text.parse().unwrap()
}

#[test]
fn a_quantity_reads_without_its_leading_zeros_and_writes_without_them() {
    assert_eq!(U256::from_quantity("0x0"), Ok(U256::ZERO));
    assert_eq!(U256::from_quantity("0x"), Ok(U256::ZERO), "an empty quantity is zero");
    assert_eq!(U256::from_quantity("0x3e7"), Ok(U256::from(999u64)));
    assert_eq!(U256::from_quantity("0X3E7"), Ok(U256::from(999u64)));
    // eth_getBalance of 0x2222...2222 on mainnet.
    let held = U256::from_quantity("0x30fc0c21318a936a96a51a5").unwrap();
    assert_eq!(held.to_string(), "947499191167351653726310821");
    assert_eq!(held.to_quantity(), "0x30fc0c21318a936a96a51a5");
    // A 32-byte word, as eth_call answers decimals().
    let word = U256::from_quantity("0x0000000000000000000000000000000000000000000000000000000000000006").unwrap();
    assert_eq!(word, U256::from(6u8));
    assert_eq!(U256::ZERO.to_quantity(), "0x0");
    assert_eq!(U256::from(999u64).to_quantity(), "0x3e7");
    let max = U256::from_quantity(&format!("0x{}", "f".repeat(64))).unwrap();
    assert_eq!(max, U256::MAX);
    assert_eq!(max.to_string(), MAX_DECIMAL);
}

#[test]
fn what_is_not_a_quantity_or_does_not_fit_is_refused() {
    assert!(matches!(U256::from_quantity(&format!("0x1{}", "0".repeat(64))), Err(BadQuantity::TooLarge(_))));
    assert!(matches!(U256::from_quantity("12"), Err(BadQuantity::NotHex(_))), "a quantity is prefixed");
    assert!(matches!(U256::from_quantity("0xzz"), Err(BadQuantity::NotHex(_))));
    assert!(matches!(U256::from_quantity(""), Err(BadQuantity::NotHex(_))));
}

#[test]
fn decimals_cover_the_full_width_and_no_more() {
    assert_eq!(big(MAX_DECIMAL), U256::MAX);
    assert_eq!(U256::from_decimal("0"), Ok(U256::ZERO));
    assert_eq!(
        big("57896044618658097711785492504343953926634992332820282019728792003956564832313").to_quantity(),
        "0x8000000000000000000000000000000000000000000000000000000000003039"
    );
    assert!(matches!(
        U256::from_decimal("115792089237316195423570985008687907853269984665640564039457584007913129639936"),
        Err(BadQuantity::TooLarge(_))
    ));
    for bad in ["", "-1", "1.5", "1e3", " "] {
        assert!(U256::from_decimal(bad).is_err(), "{bad:?}");
    }
    assert_eq!(big("0x3e7"), U256::from(999u64), "FromStr takes hex with its prefix too");
}

#[test]
fn a_quantity_is_a_json_string_both_ways() {
    let read: U256 = serde_json::from_str("\"0x3e7\"").unwrap();
    assert_eq!(read, U256::from(999u64));
    assert_eq!(serde_json::to_string(&read).unwrap(), "\"0x3e7\"");
    assert!(serde_json::from_str::<U256>("999").is_err(), "a bare number is not a quantity");
}
