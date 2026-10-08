//! System addresses: where a spot token crosses between HyperCore and
//! HyperEVM. A send to one leaves Core rather than reaching a person, so a
//! transfer checks for them and a HyperEVM transaction aims at them.

use crate::address::Address;

/// HYPE's system address, `0x2222…2222`: HYPE is the native coin on HyperEVM.
pub const HYPE_SYSTEM: Address = Address::from_bytes([0x22; 20]);

/// The address a linked spot token crosses through: `0x20`, then the token
/// index big-endian in the low bytes. Token 200 is `0x2000…00c8`.
#[must_use]
pub fn system_address(index: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[0] = 0x20;
    bytes[12..].copy_from_slice(&index.to_be_bytes());
    Address::from_bytes(bytes)
}

/// Whether a send to `address` crosses between Core and HyperEVM rather than
/// reaching a person: `0x2222…2222`, or `0x20`, zeros and a token index.
#[must_use]
pub fn is_system_address(address: Address) -> bool {
    address == HYPE_SYSTEM || system_token(address).is_some()
}

/// The token index a `0x20` system address stands for; `None` for HYPE's,
/// which crosses natively.
#[must_use]
pub fn system_token(address: Address) -> Option<u64> {
    let bytes = address.bytes();
    if bytes[0] != 0x20 || bytes[1..12].iter().any(|byte| *byte != 0) {
        return None;
    }
    let mut index = [0u8; 8];
    index.copy_from_slice(&bytes[12..]);
    Some(u64::from_be_bytes(index))
}

#[cfg(test)]
#[path = "tests/system.rs"]
mod tests;
