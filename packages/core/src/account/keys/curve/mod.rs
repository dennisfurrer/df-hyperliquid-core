//! The secp256k1 arithmetic behind [`super::LocalKey`] and [`super::recover`]: k256 (pure Rust)
//! in every build, or libsecp256k1 (C) with the `secp256k1` feature on a native target.
//! [`backend`] is the one this build uses. Both follow RFC 6979, give low-s signatures and refuse
//! the same signatures in recovery, so a build signs and recovers to the same bytes with either;
//! the tests hold the two together.

#[cfg(any(test, fuzzing, not(all(feature = "secp256k1", not(target_arch = "wasm32")))))]
pub(crate) mod k256_backend;
#[cfg(all(feature = "secp256k1", not(target_arch = "wasm32")))]
pub(crate) mod libsecp_backend;

#[cfg(not(all(feature = "secp256k1", not(target_arch = "wasm32"))))]
pub(crate) use k256_backend as backend;
#[cfg(all(feature = "secp256k1", not(target_arch = "wasm32")))]
pub(crate) use libsecp_backend as backend;

use k256::ecdsa::{RecoveryId, Signature as CurveSignature, SigningKey};

use crate::account::keys::signer::{SignError, Signature, V_OFFSET};
use crate::utils::address::Address;
use crate::utils::sign::keccak::keccak256;

/// The secp256k1 group order n, big-endian.
#[cfg(all(feature = "secp256k1", not(target_arch = "wasm32")))]
pub(crate) const ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc,
    0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

/// RFC 6979 and low-s through k256.
pub(crate) fn sign_with_k256(key: &SigningKey, digest: &[u8; 32]) -> Result<Signature, SignError> {
    let (signature, recovery_id): (CurveSignature, RecoveryId) =
        key.sign_prehash_recoverable(digest).map_err(|_| SignError::Sign)?;
    let mut compact = [0u8; 64];
    compact.copy_from_slice(&signature.to_bytes());
    Ok(signature_of(&compact, recovery_id.to_byte()))
}

/// The last twenty bytes of `keccak256(x || y)`: the address of a public key.
pub(crate) fn address_of_coordinates(coordinates: &[u8]) -> Address {
    let hash = keccak256(coordinates);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Address::from_bytes(bytes)
}

/// A signature from `r || s` and a recovery id (0 to 3).
pub(crate) fn signature_of(compact: &[u8; 64], recovery_id: u8) -> Signature {
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&compact[..32]);
    s.copy_from_slice(&compact[32..]);
    Signature::from_parts(r, s, recovery_id + V_OFFSET)
}

/// What both backends read first, with k256, so both refuse a bad `v` or an `r` or `s` that is
/// zero or past the group order with the same error.
pub(crate) fn read(signature: &Signature) -> Result<(CurveSignature, RecoveryId), SignError> {
    let v = signature.v();
    let recovery_id = signature.recovery_id().and_then(RecoveryId::from_byte).ok_or(SignError::BadV(v))?;
    let curve = CurveSignature::from_slice(&signature.to_compact()).map_err(|_| SignError::Malformed)?;
    Ok((curve, recovery_id))
}

#[cfg(all(test, feature = "secp256k1", not(target_arch = "wasm32")))]
#[path = "tests/backends.rs"]
mod tests;
