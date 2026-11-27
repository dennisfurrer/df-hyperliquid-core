//! Signing and recovery through k256, pure Rust: every build without the native `secp256k1`
//! feature, and wasm always.

use k256::ecdsa::{SigningKey, VerifyingKey};

use crate::account::keys::curve::{address_of_coordinates, read, sign_with_k256};
use crate::account::keys::signer::{SignError, Signature};
use crate::utils::address::Address;

/// Nothing beyond the k256 key [`super::super::LocalKey`] already holds.
pub(crate) struct Key;

impl Key {
    pub(crate) const fn new(_key: &SigningKey) -> Result<Key, SignError> {
        Ok(Key)
    }

    /// RFC 6979, low-s.
    pub(crate) fn sign(&self, key: &SigningKey, digest: &[u8; 32]) -> Result<Signature, SignError> {
        sign_with_k256(key, digest)
    }
}

/// The address that signed `digest`. A high `s` does not recover: k256 checks the recovered key
/// against the signature, and that check takes only low-s.
pub(crate) fn recover(digest: &[u8; 32], signature: &Signature) -> Result<Address, SignError> {
    let (curve, recovery_id) = read(signature)?;
    let key = VerifyingKey::recover_from_prehash(digest, &curve, recovery_id).map_err(|_| SignError::Unrecoverable)?;
    let point = key.to_encoded_point(false);
    Ok(address_of_coordinates(point.as_bytes().get(1..).unwrap_or_default()))
}
