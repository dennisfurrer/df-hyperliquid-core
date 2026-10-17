//! Exact 256-bit integers, the EVM's word: balances and fees in wei, gas
//! prices, ABI integers, and the native coin's 18 decimals. JSON-RPC writes
//! them as quantities, `0x` and hex digits without leading zeros. Nothing here
//! passes through a float.

use std::{fmt, str::FromStr};

use crate::utils::{Address, Dec, Round, hex};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// HYPE on HyperEVM and ETH on Arbitrum both have 18 decimals.
pub const NATIVE_DECIMALS: u32 = 18;

/// An unsigned 256-bit integer. The limbs are most significant first, so the
/// derived order is the numeric one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct U256([u64; 4]);

/// Thirty-two bytes: a transaction or block hash, a log topic, a storage slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct B256(pub [u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadQuantity {
    #[error("`{0}` is not a hex quantity; expected 0x and hex digits")]
    NotHex(String),
    #[error("`{0}` is not a whole decimal number")]
    NotDecimal(String),
    #[error("`{0}` does not fit in 256 bits")]
    TooLarge(String),
}

/// Why an amount does not convert between wei and a [`Dec`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadAmount {
    #[error("{0} is not a positive amount")]
    NotPositive(Dec),
    #[error("{value} has more than {decimals} decimal places")]
    TooFine { value: Dec, decimals: u32 },
    #[error("the amount is larger than this conversion can count")]
    TooLarge,
    /// A token counted in more than eighteen places, and an amount that needs
    /// them: a [`Dec`] keeps eighteen.
    #[error("{amount} at {decimals} decimals needs more places than a Dec keeps")]
    Unkept { amount: U256, decimals: u32 },
}

/// The places a [`Dec`] keeps.
const DEC_PLACES: u32 = crate::utils::dec::MAX_SCALE;

/// The largest power of ten in a `u64`, the step decimal text is cut in.
const TEN_19: u64 = 10_000_000_000_000_000_000;

impl U256 {
    pub const ZERO: U256 = U256([0; 4]);
    pub const ONE: U256 = U256([0, 0, 0, 1]);
    pub const MAX: U256 = U256([u64::MAX; 4]);

    #[must_use]
    pub const fn from_u64(value: u64) -> U256 {
        U256([0, 0, 0, value])
    }

    #[must_use]
    pub const fn from_u128(value: u128) -> U256 {
        U256([0, 0, (value >> 64) as u64, value as u64])
    }

    #[must_use]
    pub fn from_be_bytes(bytes: [u8; 32]) -> U256 {
        let mut limbs = [0u64; 4];
        for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks_exact(8)) {
            let mut eight = [0u8; 8];
            eight.copy_from_slice(chunk);
            *limb = u64::from_be_bytes(eight);
        }
        U256(limbs)
    }

    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (chunk, limb) in bytes.chunks_exact_mut(8).zip(self.0) {
            chunk.copy_from_slice(&limb.to_be_bytes());
        }
        bytes
    }

    /// Up to 32 big-endian bytes, a shorter run read as if zero-padded on the left.
    #[must_use]
    pub fn from_be_slice(bytes: &[u8]) -> Option<U256> {
        let start = 32usize.checked_sub(bytes.len())?;
        let mut word = [0u8; 32];
        word[start..].copy_from_slice(bytes);
        Some(U256::from_be_bytes(word))
    }

    /// The value as big-endian bytes with no leading zeros; zero is empty.
    #[must_use]
    pub fn to_be_trimmed(self) -> Vec<u8> {
        let bytes = self.to_be_bytes();
        let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(32);
        bytes[first..].to_vec()
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }

    #[must_use]
    pub const fn to_u64(self) -> Option<u64> {
        match self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 {
            true => Some(self.0[3]),
            false => None,
        }
    }

    #[must_use]
    pub const fn to_u128(self) -> Option<u128> {
        match self.0[0] == 0 && self.0[1] == 0 {
            true => Some(((self.0[2] as u128) << 64) | self.0[3] as u128),
            false => None,
        }
    }

    /// How many bits the value needs: 0 for zero, 256 for [`U256::MAX`].
    #[must_use]
    pub fn bits(self) -> u32 {
        let mut above = 256;
        for limb in self.0 {
            if limb != 0 {
                return above - limb.leading_zeros();
            }
            above -= 64;
        }
        0
    }

    #[must_use]
    pub fn checked_add(self, other: U256) -> Option<U256> {
        let mut out = [0u64; 4];
        let mut carry = false;
        for at in (0..4).rev() {
            let (sum, first) = self.0[at].overflowing_add(other.0[at]);
            let (sum, second) = sum.overflowing_add(u64::from(carry));
            out[at] = sum;
            carry = first || second;
        }
        (!carry).then_some(U256(out))
    }

    #[must_use]
    pub fn checked_sub(self, other: U256) -> Option<U256> {
        let mut out = [0u64; 4];
        let mut borrow = false;
        for at in (0..4).rev() {
            let (difference, first) = self.0[at].overflowing_sub(other.0[at]);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            out[at] = difference;
            borrow = first || second;
        }
        (!borrow).then_some(U256(out))
    }

    #[must_use]
    pub fn checked_mul(self, other: U256) -> Option<U256> {
        // Little-endian limbs for the schoolbook product; anything past four is overflow.
        let a = self.little();
        let b = other.little();
        let mut product = [0u64; 8];
        for (i, &left) in a.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &right) in b.iter().enumerate() {
                let wide = u128::from(left) * u128::from(right) + u128::from(product[i + j]) + carry;
                product[i + j] = wide as u64;
                carry = wide >> 64;
            }
            product[i + 4] = carry as u64;
        }
        if product[4..].iter().any(|limb| *limb != 0) {
            return None;
        }
        Some(U256([product[3], product[2], product[1], product[0]]))
    }

    #[must_use]
    pub fn saturating_add(self, other: U256) -> U256 {
        self.checked_add(other).unwrap_or(U256::MAX)
    }

    #[must_use]
    pub fn saturating_mul(self, other: U256) -> U256 {
        self.checked_mul(other).unwrap_or(U256::MAX)
    }

    /// `10^power`, while it fits: up to `10^77`.
    #[must_use]
    pub fn ten_to(power: u32) -> Option<U256> {
        let ten = U256::from_u64(10);
        (0..power).try_fold(U256::ONE, |value, _| value.checked_mul(ten))
    }

    /// The quotient and remainder of a division by a nonzero `u64`.
    #[must_use]
    pub fn div_rem_u64(self, divisor: u64) -> Option<(U256, u64)> {
        if divisor == 0 {
            return None;
        }
        let mut quotient = [0u64; 4];
        let mut remainder = 0u128;
        for (at, limb) in self.0.iter().enumerate() {
            let wide = (remainder << 64) | u128::from(*limb);
            quotient[at] = (wide / u128::from(divisor)) as u64;
            remainder = wide % u128::from(divisor);
        }
        Some((U256(quotient), remainder as u64))
    }

    /// The quotient and remainder of a division by a nonzero divisor, by long
    /// division one bit at a time.
    #[must_use]
    pub fn div_rem(self, divisor: U256) -> Option<(U256, U256)> {
        if divisor.is_zero() {
            return None;
        }
        let mut quotient = U256::ZERO;
        let mut remainder = U256::ZERO;
        for bit in (0..256).rev() {
            remainder = remainder.shifted_left_one();
            if self.bit(bit) {
                remainder.0[3] |= 1;
            }
            if remainder >= divisor {
                remainder = remainder.checked_sub(divisor)?;
                quotient.set_bit(bit);
            }
        }
        Some((quotient, remainder))
    }

    fn little(self) -> [u64; 4] {
        [self.0[3], self.0[2], self.0[1], self.0[0]]
    }

    fn bit(self, at: usize) -> bool {
        (self.0[3 - at / 64] >> (at % 64)) & 1 == 1
    }

    fn set_bit(&mut self, at: usize) {
        self.0[3 - at / 64] |= 1 << (at % 64);
    }

    fn shifted_left_one(self) -> U256 {
        let mut out = [0u64; 4];
        for (at, limb) in out.iter_mut().enumerate() {
            let carried = self.0.get(at + 1).map_or(0, |next| next >> 63);
            *limb = (self.0[at] << 1) | carried;
        }
        U256(out)
    }

    /// `0x` and hex digits, as JSON-RPC writes a quantity. Leading zeros are
    /// read (`eth_call` answers a whole word) and `0x` alone is zero.
    pub fn from_quantity(text: &str) -> Result<U256, BadQuantity> {
        let bad = || BadQuantity::NotHex(text.to_owned());
        let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).ok_or_else(bad)?;
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let digits = digits.trim_start_matches('0');
        if digits.len() > 64 {
            return Err(BadQuantity::TooLarge(text.to_owned()));
        }
        let bytes: [u8; 32] = hex::decode_array(&format!("{digits:0>64}")).map_err(|_| bad())?;
        Ok(U256::from_be_bytes(bytes))
    }

    /// `0x` and the hex digits without leading zeros; zero is `0x0`.
    #[must_use]
    pub fn to_quantity(self) -> String {
        let digits = hex::encode(&self.to_be_bytes());
        let trimmed = digits.trim_start_matches('0');
        match trimmed.is_empty() {
            true => "0x0".to_owned(),
            false => format!("0x{trimmed}"),
        }
    }

    /// Decimal digits and nothing else.
    pub fn from_decimal(text: &str) -> Result<U256, BadQuantity> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(BadQuantity::NotDecimal(text.to_owned()));
        }
        let ten = U256::from_u64(10);
        text.bytes().try_fold(U256::ZERO, |value, digit| {
            value
                .checked_mul(ten)
                .and_then(|value| value.checked_add(U256::from_u64(u64::from(digit - b'0'))))
                .ok_or_else(|| BadQuantity::TooLarge(text.to_owned()))
        })
    }

    /// The amount as a decimal of `decimals` places: wei at 18 is the coin.
    /// Exact, or refused: past eighteen places the digits a [`Dec`] cannot
    /// keep must be zero.
    pub fn to_dec(self, decimals: u32) -> Result<Dec, BadAmount> {
        let (units, scale) = match decimals.checked_sub(DEC_PLACES) {
            None | Some(0) => (self, decimals),
            Some(extra) => {
                let unkept = BadAmount::Unkept { amount: self, decimals };
                let step = U256::ten_to(extra).ok_or_else(|| unkept.clone())?;
                let (quotient, rest) = self.div_rem(step).ok_or(BadAmount::TooLarge)?;
                if !rest.is_zero() {
                    return Err(unkept);
                }
                (quotient, DEC_PLACES)
            }
        };
        let units = units.to_u128().and_then(|units| i128::try_from(units).ok()).ok_or(BadAmount::TooLarge)?;
        Dec::new(units, scale).ok_or(BadAmount::TooLarge)
    }

    /// `value` counted in `10^-decimals`: 1.5 at 18 is 1.5e18 wei. A value
    /// finer than `decimals` is refused, never rounded.
    pub fn from_dec(value: Dec, decimals: u32) -> Result<U256, BadAmount> {
        if value.is_negative() {
            return Err(BadAmount::NotPositive(value));
        }
        let places = value.places();
        if places > decimals {
            return Err(BadAmount::TooFine { value, decimals });
        }
        let units = value
            .units_at(places, Round::Down)
            .and_then(|units| u128::try_from(units).ok())
            .ok_or(BadAmount::TooLarge)?;
        U256::ten_to(decimals - places)
            .and_then(|step| U256::from_u128(units).checked_mul(step))
            .ok_or(BadAmount::TooLarge)
    }
}

impl From<u8> for U256 {
    fn from(value: u8) -> U256 {
        U256::from_u64(u64::from(value))
    }
}

impl From<u16> for U256 {
    fn from(value: u16) -> U256 {
        U256::from_u64(u64::from(value))
    }
}

impl From<u32> for U256 {
    fn from(value: u32) -> U256 {
        U256::from_u64(u64::from(value))
    }
}

impl From<u64> for U256 {
    fn from(value: u64) -> U256 {
        U256::from_u64(value)
    }
}

impl From<u128> for U256 {
    fn from(value: u128) -> U256 {
        U256::from_u128(value)
    }
}

impl From<B256> for U256 {
    fn from(word: B256) -> U256 {
        U256::from_be_bytes(word.0)
    }
}

/// Decimal, the way a person reads it.
impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut chunks = Vec::new();
        let mut left = *self;
        while let Some((quotient, chunk)) = left.div_rem_u64(TEN_19).filter(|_| !left.is_zero()) {
            chunks.push(chunk);
            left = quotient;
        }
        let Some((first, rest)) = chunks.split_last() else {
            return f.write_str("0");
        };
        write!(f, "{first}")?;
        for chunk in rest.iter().rev() {
            write!(f, "{chunk:019}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U256({self})")
    }
}

/// A quantity when it starts with `0x`, decimal digits otherwise.
impl FromStr for U256 {
    type Err = BadQuantity;

    fn from_str(text: &str) -> Result<U256, BadQuantity> {
        let text = text.trim();
        match text.starts_with("0x") || text.starts_with("0X") {
            true => U256::from_quantity(text),
            false => U256::from_decimal(text),
        }
    }
}

impl Serialize for U256 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_quantity())
    }
}

/// A quantity string. A bare JSON number is refused: it may have been a float.
impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<U256, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        U256::from_quantity(&text).map_err(de::Error::custom)
    }
}

impl B256 {
    pub const ZERO: B256 = B256([0u8; 32]);

    /// An address as a word: twelve zero bytes, then its twenty.
    #[must_use]
    pub fn from_address(address: Address) -> B256 {
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(address.bytes());
        B256(word)
    }

    /// The address in the low twenty bytes, when the high twelve are zero:
    /// how an indexed `address` sits in a log topic.
    #[must_use]
    pub fn to_address(&self) -> Option<Address> {
        if self.0[..12].iter().any(|byte| *byte != 0) {
            return None;
        }
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.0[12..]);
        Some(Address::from_bytes(bytes))
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode_prefixed(&self.0)
    }
}

impl From<[u8; 32]> for B256 {
    fn from(bytes: [u8; 32]) -> B256 {
        B256(bytes)
    }
}

impl From<U256> for B256 {
    fn from(value: U256) -> B256 {
        B256(value.to_be_bytes())
    }
}

impl FromStr for B256 {
    type Err = crate::utils::BadHex;

    fn from_str(text: &str) -> Result<B256, Self::Err> {
        hex::decode_array(text).map(B256)
    }
}

impl fmt::Display for B256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for B256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B256({})", self.to_hex())
    }
}

impl Serialize for B256 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for B256 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<B256, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/u256.rs"]
mod tests;
