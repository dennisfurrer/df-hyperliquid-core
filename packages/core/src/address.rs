use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Twenty bytes. Read in either case, written lowercase, which is the only
/// form the venue hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Address([u8; 20]);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a 20-byte hex address")]
pub struct BadAddress(String);

impl Address {
    pub const ZERO: Address = Address([0u8; 20]);

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Address(bytes)
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8; 20] {
        &self.0
    }

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
        crate::hex::encode_prefixed(&self.0)
    }
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
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
mod tests {
    use super::*;

    #[test]
    fn an_address_reads_back_out_of_its_own_json() {
        let address: Address = "0xbaff87940cb148c5e2529c0a5944b0a3386fb015".parse().unwrap();
        let json = serde_json::to_string(&address).unwrap();
        assert_eq!(json, "\"0xbaff87940cb148c5e2529c0a5944b0a3386fb015\"");
        let back: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(back, address);
    }

    #[test]
    fn any_case_reads_to_the_same_address_and_writes_lowercase() {
        let upper: Address = serde_json::from_str("\"0xBAFF87940CB148C5E2529C0A5944B0A3386FB015\"").unwrap();
        let mixed: Address = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".parse().unwrap();
        let lower: Address = serde_json::from_str("\"0xbaff87940cb148c5e2529c0a5944b0a3386fb015\"").unwrap();
        assert_eq!(upper, lower);
        assert_eq!(upper.to_string(), "0xbaff87940cb148c5e2529c0a5944b0a3386fb015");
        assert_eq!(mixed.to_hex(), "0xaf88d065e77c8cc2239327c5edb3a432268e5831");
        assert_eq!(" 0X14791697260E4c9A71f18484C9f997B308e59325 ".parse::<Address>().unwrap().bytes()[0], 0x14);
    }

    #[test]
    fn anything_else_is_refused_rather_than_zeroed() {
        for bad in [
            "0xdeadbeef",
            "baff87940cb148c5e2529c0a5944b0a3386fb015",
            "0xzzff87940cb148c5e2529c0a5944b0a3386fb015",
            "0xbaff87940cb148c5e2529c0a5944b0a3386fb0155",
            "",
            "0x",
        ] {
            assert!(bad.parse::<Address>().is_err(), "{bad} should be refused");
        }
        for said in ["\"0x00\"", "\"nonsense\"", "\"\"", "42"] {
            assert!(serde_json::from_str::<Address>(said).is_err(), "{said}");
        }
    }
}
