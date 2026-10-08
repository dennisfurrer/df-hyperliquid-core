//! Expected values are node's `BigInt` over the same digits, and live
//! answers from `vectors/hyperevm_mainnet.json`.

use super::*;

const MAX_DECIMAL: &str = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

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
fn arithmetic_is_checked_at_the_edges() {
    assert_eq!(U256::MAX.checked_add(U256::ONE), None);
    assert_eq!(U256::ZERO.checked_sub(U256::ONE), None);
    assert_eq!(U256::MAX.saturating_add(U256::ONE), U256::MAX);
    let two_128 = big("340282366920938463463374607431768211456");
    assert_eq!(two_128.checked_mul(two_128), None, "2^256 does not fit");
    let two_200 = two_128.checked_mul(U256::from(1u128 << 72)).unwrap();
    assert_eq!(two_200.to_string(), "1606938044258990275541962092341162602522202993782792835301376");
    assert_eq!(
        U256::ten_to(77).unwrap().to_quantity(),
        "0xdd15fe86affad91249ef0eb713f39ebeaa987b6e6fd2a0000000000000000000"
    );
    assert_eq!(U256::ten_to(78), None);
    assert_eq!(U256::from(u64::MAX).checked_add(U256::ONE).unwrap().to_quantity(), "0x10000000000000000");
    assert_eq!(big("0x10000000000000000").checked_sub(U256::ONE), Some(U256::from(u64::MAX)));
    assert_eq!(U256::MAX.saturating_mul(U256::from(2u8)), U256::MAX);
}

#[test]
fn bytes_and_narrow_integers_convert_both_ways() {
    let mut word = [0u8; 32];
    word[0] = 0x80;
    word[31] = 0x01;
    let value = U256::from_be_bytes(word);
    assert_eq!(value.to_be_bytes(), word);
    assert_eq!(U256::from_be_slice(&[0x01, 0x00]), Some(U256::from(256u16)));
    assert_eq!(U256::from_be_slice(&[]), Some(U256::ZERO));
    assert_eq!(U256::from_be_slice(&[0u8; 33]), None, "33 bytes is more than a word");
    assert_eq!(U256::from(u64::MAX).to_u64(), Some(u64::MAX));
    assert_eq!(U256::from(u128::MAX).to_u64(), None);
    assert_eq!(U256::from(u128::MAX).to_u128(), Some(u128::MAX));
    assert_eq!(big("340282366920938463463374607431768211456").to_u128(), None);
    assert!(U256::ONE < U256::MAX && U256::from(1u128 << 64) > U256::from(u64::MAX));
    assert_eq!(U256::ZERO.bits(), 0);
    assert_eq!(U256::from(255u8).bits(), 8);
    assert_eq!(U256::MAX.bits(), 256);
}

#[test]
fn wei_converts_to_and_from_an_exact_decimal_at_a_named_number_of_places() {
    // HYPE on HyperEVM and ETH on Arbitrum have eighteen; USDC on Arbitrum six.
    assert_eq!(U256::from_dec(dec("1.5"), 18), Ok(U256::from(1_500_000_000_000_000_000u64)));
    assert_eq!(U256::from(1_500_000_000_000_000_000u64).to_dec(18), Ok(dec("1.5")));
    assert_eq!(U256::ONE.to_dec(18), Ok(dec("0.000000000000000001")));
    assert_eq!(U256::from_dec(dec("12.5"), 6), Ok(U256::from(12_500_000u64)));
    assert_eq!(U256::from_dec(Dec::ZERO, 18), Ok(U256::ZERO));
    // The balance read live: 947499191.167351653726310821 HYPE.
    assert_eq!(big("947499191167351653726310821").to_dec(18), Ok(dec("947499191.167351653726310821")));
    // Past eighteen places, as a token with extra EVM decimals counts: exact or refused.
    assert_eq!(U256::ten_to(26).unwrap().to_dec(26), Ok(Dec::ONE));
    assert_eq!(U256::from_dec(Dec::ONE, 26), Ok(U256::ten_to(26).unwrap()));
    assert_eq!(U256::ONE.to_dec(26), Err(BadAmount::Unkept { amount: U256::ONE, decimals: 26 }));
    assert_eq!(U256::from_dec(Dec::ONE, 77), U256::ten_to(77).ok_or(BadAmount::TooLarge));
    assert_eq!(U256::from_dec(dec("10"), 77), Err(BadAmount::TooLarge));
    assert_eq!(U256::MAX.to_dec(0), Err(BadAmount::TooLarge), "past what a Dec counts");
    assert_eq!(U256::from_dec(dec("0.0000001"), 6), Err(BadAmount::TooFine { value: dec("0.0000001"), decimals: 6 }));
    assert_eq!(U256::from_dec(dec("-1"), 18), Err(BadAmount::NotPositive(dec("-1"))));
}

#[test]
fn a_quantity_is_a_json_string_both_ways() {
    let read: U256 = serde_json::from_str("\"0x3e7\"").unwrap();
    assert_eq!(read, U256::from(999u64));
    assert_eq!(serde_json::to_string(&read).unwrap(), "\"0x3e7\"");
    assert!(serde_json::from_str::<U256>("999").is_err(), "a bare number is not a quantity");
}

#[test]
fn a_word_is_thirty_two_bytes_of_hex() {
    let text = "0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3";
    let word: B256 = text.parse().unwrap();
    assert_eq!(word.to_string(), text);
    assert_eq!(word.0[0], 0x8c);
    assert_eq!(serde_json::from_str::<B256>(&format!("\"{text}\"")).unwrap(), word);
    assert_eq!(serde_json::to_string(&word).unwrap(), format!("\"{text}\""));
    assert!("0x8c7f".parse::<B256>().is_err());
    let user: B256 = "0x0000000000000000000000006b9e773128f453f5c2c60935ee2de2cbc5390a24".parse().unwrap();
    assert_eq!(user.to_address().unwrap().to_hex(), "0x6b9e773128f453f5c2c60935ee2de2cbc5390a24");
    assert_eq!(B256::from_address(user.to_address().unwrap()), user);
    assert_eq!(word.to_address(), None, "high bytes set: not an address");
    assert_eq!(U256::from(word).to_be_bytes(), word.0);
    assert_eq!(B256::from(U256::from(6u8)).0[31], 6);
}
