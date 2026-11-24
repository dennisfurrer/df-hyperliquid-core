//! A client order id: 128 bits the caller picks, written as `0x` and 32 hex digits, on an order
//! as it is placed and on the order and fills read back.

use std::{borrow::Cow, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

use crate::utils::hex::{self, BadHex};

/// A client order id: 128 bits, written as `0x` and 32 hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cloid([u8; 16]);

impl Cloid {
    /// From its 16 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Cloid {
        Cloid(bytes)
    }

    /// From a number, its bytes big-endian.
    #[must_use]
    pub const fn from_u128(value: u128) -> Cloid {
        Cloid(value.to_be_bytes())
    }

    /// Its 16 bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// As written on the wire: `0x` and 32 hex digits.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode_prefixed(&self.0)
    }
}

impl FromStr for Cloid {
    type Err = BadHex;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        hex::decode_array(text).map(Cloid)
    }
}

impl fmt::Display for Cloid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// From the hex the venue writes for an order's `cloid`.
impl<'de> Deserialize<'de> for Cloid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Cow<'de, str>>::deserialize(deserializer)?.parse().map_err(de::Error::custom)
    }
}
