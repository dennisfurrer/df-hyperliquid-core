//! Typed text read into its kind at the edges of each width, and shown back.

use super::*;

#[test]
fn typed_text_is_range_checked_and_shown_back() {
    assert!(Token::parse(&Kind::Uint(8), "x", "256").is_err());
    assert!(Token::parse(&Kind::Uint(8), "x", "255").is_ok());
    assert!(Token::parse(&Kind::Int(8), "x", "-128").is_ok());
    assert!(Token::parse(&Kind::Int(8), "x", "128").is_err());
    assert!(Token::parse(&Kind::Int(8), "x", "-129").is_err());
    assert!(Token::parse(&Kind::Address, "x", "0x12").is_err());
    assert_eq!(Token::parse(&Kind::Int(64), "x", "-12345").unwrap(), Token::int(-12345));
    let min = "-57896044618658097711785492504343953926634992332820282019728792003956564819968";
    let int256 = Token::parse(&Kind::Int(256), "x", min).unwrap();
    assert_eq!(int256.show(&Kind::Int(256)), min);
    assert_eq!(Token::int(i128::MIN).as_i128(), Some(i128::MIN));
    assert_eq!(Token::int(-1).as_i64(), Some(-1));
    assert_eq!(int256.as_i128(), None, "past what an i128 holds");
    assert_eq!(Token::uint(u64::MAX).as_u64(), Some(u64::MAX));
    assert_eq!(Token::uint(u64::MAX).as_u32(), None);
    let error = Token::parse(&Kind::Uint(64), "limitPx", "1.5").unwrap_err();
    assert_eq!(error.to_string(), "limitPx must be a whole number that fits uint64, and `1.5` is not");
}
