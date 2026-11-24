//! secp256k1 signatures as the venue reads them, recovery, and the [`Signer`] trait: a browser
//! wallet through its typed-data JSON, or a local key. Without the `sign` feature a [`Signature`]
//! can still be posted, but nothing can make or check one.

#[cfg(feature = "sign")]
use std::fmt;
use std::future::Future;

#[cfg(feature = "sign")]
use k256::ecdsa::{RecoveryId, Signature as CurveSignature, SigningKey, VerifyingKey};
use serde::{Serialize, Serializer, ser::SerializeStruct};
#[cfg(feature = "sign")]
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::utils::address::Address;
#[cfg(feature = "sign")]
use crate::utils::hex;
use crate::utils::json::Json;
use crate::utils::sign::eip712::{Domain, TypedStruct};
#[cfg(feature = "sign")]
use crate::utils::sign::keccak::keccak256;

const V_OFFSET: u8 = 27;

/// `{r, s, v}`: low-s, `v = 27 + recovery id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

/// Why a signature could not be made, read or checked, or a key could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SignError {
    /// The key failed to sign the digest.
    #[error("the key could not sign this digest")]
    Sign,
    /// The text is not `0x` and 65 bytes of hex.
    #[error("a signature is 65 bytes of hex")]
    NotASignature,
    /// `r` and `s` do not form a valid secp256k1 signature.
    #[error("the signature components are not a valid curve point")]
    Malformed,
    /// No public key recovers from the signature and digest.
    #[error("no public key recovers from this signature")]
    Unrecoverable,
    /// The recovery byte `v` is not 0, 1, 27 or 28.
    #[error("`{0}` is not an Ethereum recovery parameter; expected 27 or 28")]
    BadV(u8),
    /// The signature recovered to another address than the one required.
    #[error("the signature is from {recovered}, not {expected}")]
    WrongSigner {
        /// The address the signature had to come from.
        expected: Address,
        /// The address it recovered to.
        recovered: Address,
    },
    /// The bytes are not a valid secp256k1 private key.
    #[error("the key is not a valid secp256k1 private key")]
    NotAKey,
}

impl Signature {
    /// From `r`, `s` and `v` as they are; nothing is checked or normalised.
    #[must_use]
    pub const fn from_parts(r: [u8; 32], s: [u8; 32], v: u8) -> Self {
        Signature { r, s, v }
    }

    /// What a wallet's `eth_signTypedData_v4` answers: `0x` and 65 bytes,
    /// `r || s || v`. `v` may be 0/1 or 27/28 and comes out 27/28; a high `s`
    /// is folded to its low twin, which is the same signature to the curve and
    /// the only form recovery here and the venue take.
    #[cfg(feature = "sign")]
    pub fn from_hex(text: &str) -> Result<Signature, SignError> {
        let bytes: [u8; 65] = hex::decode_array(text).map_err(|_| SignError::NotASignature)?;
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..64]);
        let parity = match bytes[64] {
            0 | 27 => 0,
            1 | 28 => 1,
            other => return Err(SignError::BadV(other)),
        };
        let compact = Signature { r, s, v: V_OFFSET + parity }.to_compact();
        let curve = CurveSignature::from_slice(&compact).map_err(|_| SignError::Malformed)?;
        Ok(match curve.normalize_s() {
            None => Signature { r, s, v: V_OFFSET + parity },
            Some(low) => {
                s.copy_from_slice(&low.to_bytes()[32..]);
                Signature { r, s, v: V_OFFSET + (parity ^ 1) }
            }
        })
    }

    /// The 32-byte `r`.
    #[must_use]
    pub const fn r(&self) -> &[u8; 32] {
        &self.r
    }

    /// The 32-byte `s`.
    #[must_use]
    pub const fn s(&self) -> &[u8; 32] {
        &self.s
    }

    /// 27 or 28.
    #[must_use]
    pub const fn v(&self) -> u8 {
        self.v
    }

    /// The recovery id, `v - 27`; `None` when `v` is below 27.
    #[must_use]
    pub const fn recovery_id(&self) -> Option<u8> {
        self.v.checked_sub(V_OFFSET)
    }

    /// `r` as `0x` and 64 hex digits.
    #[must_use]
    pub fn r_hex(&self) -> String {
        crate::utils::hex::encode_prefixed(&self.r)
    }

    /// `s` as `0x` and 64 hex digits.
    #[must_use]
    pub fn s_hex(&self) -> String {
        crate::utils::hex::encode_prefixed(&self.s)
    }

    /// `r || s`, 64 bytes, without `v`.
    #[must_use]
    pub fn to_compact(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&self.r);
        bytes[32..].copy_from_slice(&self.s);
        bytes
    }

    /// `{"r":"0x..","s":"0x..","v":27}`, as `/exchange` reads it: `r` and `s`
    /// as full 32-byte words.
    #[must_use]
    pub fn to_json(&self) -> String {
        self.json().compact()
    }

    pub(crate) fn json(&self) -> Json {
        Json::Object(vec![
            ("r".to_owned(), Json::String(self.r_hex())),
            ("s".to_owned(), Json::String(self.s_hex())),
            ("v".to_owned(), Json::Number(self.v.to_string())),
        ])
    }
}

impl Serialize for Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut signature = serializer.serialize_struct("Signature", 3)?;
        signature.serialize_field("r", &self.r_hex())?;
        signature.serialize_field("s", &self.s_hex())?;
        signature.serialize_field("v", &self.v)?;
        signature.end()
    }
}

/// The address that signed `digest`.
#[cfg(feature = "sign")]
pub fn recover(digest: &[u8; 32], signature: &Signature) -> Result<Address, SignError> {
    let recovery_id = signature.recovery_id().and_then(RecoveryId::from_byte).ok_or(SignError::BadV(signature.v))?;
    let curve_signature = CurveSignature::from_slice(&signature.to_compact()).map_err(|_| SignError::Malformed)?;
    let key = VerifyingKey::recover_from_prehash(digest, &curve_signature, recovery_id)
        .map_err(|_| SignError::Unrecoverable)?;
    Ok(address_of_public_key(&key))
}

/// Recovers `signature` and requires `expected`: what every wallet signature
/// passes before it is posted, since a wallet can sign as another account.
#[cfg(feature = "sign")]
pub fn verify(digest: &[u8; 32], signature: &Signature, expected: Address) -> Result<(), SignError> {
    let recovered = recover(digest, signature)?;
    match recovered == expected {
        true => Ok(()),
        false => Err(SignError::WrongSigner { expected, recovered }),
    }
}

/// `keccak256(uncompressed public key without its 0x04 tag)`, last twenty bytes.
#[cfg(feature = "sign")]
#[must_use]
pub fn address_of_public_key(key: &VerifyingKey) -> Address {
    let point = key.to_encoded_point(false);
    let coordinates = &point.as_bytes()[1..];
    let hash = keccak256(coordinates);
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(&hash[12..]);
    Address::from_bytes(bytes)
}

/// Whoever signs for an account. In the browser that is the wallet, handed
/// [`TypedStruct::wallet_json`]; in tests and native tools, a [`LocalKey`].
/// No `Send` bound: the browser is single-threaded.
pub trait Signer {
    /// The address the signatures recover to.
    fn address(&self) -> Address;

    /// Signs `data` under `domain` as EIP-712 typed data.
    fn sign_typed(&self, domain: &Domain, data: &TypedStruct<'_>)
    -> impl Future<Output = Result<Signature, SignError>>;
}

/// If `k256` ever stops wiping its key on drop, this stops compiling.
#[cfg(feature = "sign")]
const _: fn() = || {
    fn wiped_on_drop<T: ZeroizeOnDrop>() {}
    wiped_on_drop::<SigningKey>();
};

/// A private key held in memory, for tests and native tools. The browser
/// never holds one: its wallet signs.
#[cfg(feature = "sign")]
pub struct LocalKey {
    key: SigningKey,
    address: Address,
}

#[cfg(feature = "sign")]
impl LocalKey {
    /// From the 32 secret bytes; [`SignError::NotAKey`] when they are not a valid key.
    pub fn from_bytes(secret: &[u8; 32]) -> Result<Self, SignError> {
        let key = SigningKey::from_bytes(secret.into()).map_err(|_| SignError::NotAKey)?;
        let address = address_of_public_key(key.verifying_key());
        Ok(LocalKey { key, address })
    }

    /// Thirty-two bytes of hex, `0x` optional. A rejected key is never quoted back.
    pub fn from_hex(secret: &str) -> Result<Self, SignError> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        hex::decode_into(secret, bytes.as_mut_slice()).map_err(|_| SignError::NotAKey)?;
        LocalKey::from_bytes(&bytes)
    }

    /// The key's address.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    /// RFC 6979, so the same digest always signs the same; low-s.
    pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<Signature, SignError> {
        let (signature, recovery_id): (CurveSignature, RecoveryId) =
            self.key.sign_prehash_recoverable(digest).map_err(|_| SignError::Sign)?;
        let bytes = signature.to_bytes();
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[..32]);
        s.copy_from_slice(&bytes[32..]);
        Ok(Signature { r, s, v: recovery_id.to_byte() + V_OFFSET })
    }
}

#[cfg(feature = "sign")]
impl Signer for LocalKey {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_typed(
        &self,
        domain: &Domain,
        data: &TypedStruct<'_>,
    ) -> impl Future<Output = Result<Signature, SignError>> {
        std::future::ready(self.sign_digest(&data.digest(domain)))
    }
}

#[cfg(feature = "sign")]
impl fmt::Debug for LocalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalKey").field("address", &self.address.to_hex()).finish_non_exhaustive()
    }
}

/// Holds because `SigningKey` wipes itself; the assertion above checks that.
#[cfg(feature = "sign")]
impl ZeroizeOnDrop for LocalKey {}

#[cfg(test)]
#[cfg(feature = "sign")]
#[path = "tests/signer.rs"]
mod tests;
