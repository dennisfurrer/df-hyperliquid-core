//! The 20-byte account [`Address`] and its `0x` hex form.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Twenty bytes. Read in either case, written lowercase, which is the only
/// form the venue hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Address([u8; 20]);

/// Text that is not `0x` and forty hex digits; holds the text given, trimmed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a 20-byte hex address")]
pub struct BadAddress(String);

impl Address {
    /// `0x0000000000000000000000000000000000000000`.
    pub const ZERO: Address = Address([0u8; 20]);

    /// The address with these 20 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Address(bytes)
    }

    /// The 20 bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Whether this is [`Address::ZERO`].
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Address::ZERO
    }

    /// For constants: `0x` and forty hex digits in either case, or `None`.
    #[must_use]
    pub const fn parse_const(text: &str) -> Option<Address> {
        let raw = text.as_bytes();
        if raw.len() != 42 || raw[0] != b'0' || (raw[1] != b'x' && raw[1] != b'X') {
            return None;
        }
        let mut bytes = [0u8; 20];
        let mut at = 0;
        while at < 20 {
            let (Some(high), Some(low)) = (hex_digit(raw[2 + at * 2]), hex_digit(raw[3 + at * 2])) else {
                return None;
            };
            bytes[at] = (high << 4) | low;
            at += 1;
        }
        Some(Address(bytes))
    }

    /// The lowercase `0x` form the venue's JSON expects.
    #[must_use]
    pub fn to_hex(self) -> String {
        crate::utils::hex::encode_prefixed(&self.0)
    }
}

/// Each byte's value as a hex digit, or [`NOT_HEX`]: one lookup a digit instead of three ranges.
const HEX: [u8; 256] = {
    let mut table = [NOT_HEX; 256];
    let mut byte = 0;
    while byte < 256 {
        table[byte] = match byte as u8 {
            digit @ b'0'..=b'9' => digit - b'0',
            digit @ b'a'..=b'f' => digit - b'a' + 10,
            digit @ b'A'..=b'F' => digit - b'A' + 10,
            _ => NOT_HEX,
        };
        byte += 1;
    }
    table
};
const NOT_HEX: u8 = 0xff;

const fn hex_digit(byte: u8) -> Option<u8> {
    match HEX[byte as usize] {
        NOT_HEX => None,
        digit => Some(digit),
    }
}

impl FromStr for Address {
    type Err = BadAddress;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let raw = text.trim();
        Address::parse_const(raw).ok_or_else(|| BadAddress(raw.to_owned()))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/address.rs"]
mod tests;
