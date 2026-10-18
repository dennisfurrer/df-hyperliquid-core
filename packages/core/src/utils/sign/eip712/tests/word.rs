//! A `uint256` word to and from its decimal digits.

use super::*;

#[test]
fn a_word_reads_and_writes_as_decimal() {
    for digits in ["0", "1", "10000000000000000000", "340282366920938463463374607431768211455"] {
        assert_eq!(word_decimal(&decimal_word(digits).unwrap()), digits);
    }
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    assert_eq!(decimal_word(max), Some([0xff; 32]));
    assert_eq!(word_decimal(&[0xff; 32]), max);
    assert_eq!(decimal_word("115792089237316195423570985008687907853269984665640564039457584007913129639936"), None);
    assert_eq!(decimal_word(""), None);
    assert_eq!(decimal_word("1e3"), None);
    assert_eq!(decimal_word("-1"), None);
}
