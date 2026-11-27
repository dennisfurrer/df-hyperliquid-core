//! Signing and recovery through libsecp256k1, the C library Bitcoin Core uses (the `secp256k1`
//! crate, compiled by the `cc` crate, so the build needs a C compiler for the target). The
//! `secp256k1` feature on a native target. It signs about twice as fast as k256 and recovers
//! about five times as fast.

use std::sync::OnceLock;

use k256::ecdsa::SigningKey;
use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
use secp256k1::{Message, Secp256k1, SecretKey, SignOnly, VerifyOnly};
use zeroize::Zeroizing;

use crate::account::keys::curve::{ORDER, address_of_coordinates, read, sign_with_k256, signature_of};
use crate::account::keys::signer::{SignError, Signature};
use crate::utils::address::Address;

/// The key as libsecp256k1 holds it, and a signing context blinded once with 32 bytes from the
/// operating system's random source: libsecp256k1 mixes them into its signing arithmetic so that
/// timing and power do not follow the key. The key is wiped when this is dropped.
pub(crate) struct Key {
    context: Secp256k1<SignOnly>,
    secret: SecretKey,
}

impl Key {
    pub(crate) fn new(key: &SigningKey) -> Result<Key, SignError> {
        let bytes = Zeroizing::new(<[u8; 32]>::from(key.to_bytes()));
        let secret = SecretKey::from_byte_array(*bytes).map_err(|_| SignError::NotAKey)?;
        let mut context = Secp256k1::signing_only();
        let mut seed = Zeroizing::new([0u8; 32]);
        if getrandom02::getrandom(seed.as_mut_slice()).is_ok() {
            context.seeded_randomize(&seed);
        }
        Ok(Key { context, secret })
    }

    /// RFC 6979, low-s. RFC 6979 reduces the digest modulo the group order n before it derives the
    /// nonce; k256 does, libsecp256k1 does not. The two differ only for a digest at or above n,
    /// which a keccak digest is about once in 2^128, and such a digest is signed by k256 here so
    /// that both backends give the same bytes for every digest.
    pub(crate) fn sign(&self, key: &SigningKey, digest: &[u8; 32]) -> Result<Signature, SignError> {
        if digest >= &ORDER {
            return sign_with_k256(key, digest);
        }
        let signed = self.context.sign_ecdsa_recoverable(Message::from_digest(*digest), &self.secret);
        let (recovery_id, compact) = signed.serialize_compact();
        let recovery_id = u8::try_from(i32::from(recovery_id)).map_err(|_| SignError::Sign)?;
        Ok(signature_of(&compact, recovery_id))
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        self.secret.non_secure_erase();
    }
}

/// The address that signed `digest`. The signature is read by k256 first ([`read`]), so a bad
/// `v` or an `r` or `s` out of range is refused as k256 refuses it. k256 also refuses to recover
/// a high `s` and libsecp256k1 does not, so a high `s` is refused here first, with k256's error.
/// The verifying context holds no secret, so it is shared and not blinded.
pub(crate) fn recover(digest: &[u8; 32], signature: &Signature) -> Result<Address, SignError> {
    let (curve, recovery_id) = read(signature)?;
    if curve.normalize_s().is_some() {
        return Err(SignError::Unrecoverable);
    }
    static VERIFY: OnceLock<Secp256k1<VerifyOnly>> = OnceLock::new();
    let context = VERIFY.get_or_init(Secp256k1::verification_only);
    let recovery_id =
        RecoveryId::try_from(i32::from(recovery_id.to_byte())).map_err(|_| SignError::BadV(signature.v()))?;
    let recoverable =
        RecoverableSignature::from_compact(&signature.to_compact(), recovery_id).map_err(|_| SignError::Malformed)?;
    let key =
        context.recover_ecdsa(Message::from_digest(*digest), &recoverable).map_err(|_| SignError::Unrecoverable)?;
    Ok(address_of_coordinates(key.serialize_uncompressed().get(1..).unwrap_or_default()))
}
