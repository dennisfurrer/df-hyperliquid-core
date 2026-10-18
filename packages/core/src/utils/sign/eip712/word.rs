//! 32-byte words: a number, an address or a bool as EIP-712 encodes it, and a `uint256` to and from
//! its decimal digits.

use crate::utils::address::Address;

/// The `uint256` word of a number that fits 128 bits.
#[must_use]
pub const fn uint256(value: u128) -> [u8; 32] {
    let bytes = value.to_be_bytes();
    let mut word = [0u8; 32];
    let mut index = 0;
    while index < 16 {
        word[16 + index] = bytes[index];
        index += 1;
    }
    word
}

/// A big-endian word as its decimal digits.
#[must_use]
pub fn word_decimal(bytes: &[u8; 32]) -> String {
    // Nineteen digits at a time: 10^19 is the largest power of ten under 2^64.
    const GROUP: u128 = 10_000_000_000_000_000_000;
    let mut limbs = [0u64; 4];
    for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks_exact(8)) {
        let mut eight = [0u8; 8];
        eight.copy_from_slice(chunk);
        *limb = u64::from_be_bytes(eight);
    }
    let mut groups = Vec::new();
    while limbs.iter().any(|limb| *limb != 0) {
        let mut remainder = 0u128;
        for limb in &mut limbs {
            let value = (remainder << 64) | u128::from(*limb);
            // The remainder is under 10^19, so the quotient is under 2^64.
            *limb = (value / GROUP) as u64;
            remainder = value % GROUP;
        }
        groups.push(remainder as u64);
    }
    let Some((first, rest)) = groups.split_last() else {
        return "0".to_owned();
    };
    let mut text = first.to_string();
    for group in rest.iter().rev() {
        text.push_str(&format!("{group:019}"));
    }
    text
}

/// Decimal digits as a big-endian word; `None` for anything else or past 2^256.
#[must_use]
pub fn decimal_word(digits: &str) -> Option<[u8; 32]> {
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut limbs = [0u64; 4];
    for byte in digits.bytes() {
        let mut carry = u128::from(byte - b'0');
        for limb in limbs.iter_mut().rev() {
            let value = u128::from(*limb) * 10 + carry;
            // The low 64 bits stay in the limb; the rest carries up.
            *limb = value as u64;
            carry = value >> 64;
        }
        if carry != 0 {
            return None;
        }
    }
    let mut word = [0u8; 32];
    for (chunk, limb) in word.chunks_exact_mut(8).zip(limbs) {
        chunk.copy_from_slice(&limb.to_be_bytes());
    }
    Some(word)
}

pub(super) fn word(value: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    bytes
}

pub(super) fn address_word(address: Address) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(address.bytes());
    bytes
}

#[cfg(test)]
#[path = "tests/word.rs"]
mod tests;
