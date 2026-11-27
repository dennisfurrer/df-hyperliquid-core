//! Who signed: the address a signature recovers to, the check that it is the expected one, and the
//! address of a public key. Reached through [`super::signer`], where they have always been.

use k256::ecdsa::VerifyingKey;

use crate::account::keys::curve::{address_of_coordinates, backend};
use crate::account::keys::signer::{SignError, Signature};
use crate::utils::address::Address;

/// The address that signed `digest`. With the `secp256k1` feature on a native target
/// libsecp256k1 recovers it, otherwise k256; both refuse the same signatures.
pub fn recover(digest: &[u8; 32], signature: &Signature) -> Result<Address, SignError> {
    backend::recover(digest, signature)
}

/// Recovers `signature` and requires `expected`: what every wallet signature
/// passes before it is posted, since a wallet can sign as another account.
pub fn verify(digest: &[u8; 32], signature: &Signature, expected: Address) -> Result<(), SignError> {
    let recovered = recover(digest, signature)?;
    match recovered == expected {
        true => Ok(()),
        false => Err(SignError::WrongSigner { expected, recovered }),
    }
}

/// `keccak256(uncompressed public key without its 0x04 tag)`, last twenty bytes.
#[must_use]
pub fn address_of_public_key(key: &VerifyingKey) -> Address {
    let point = key.to_encoded_point(false);
    address_of_coordinates(point.as_bytes().get(1..).unwrap_or_default())
}
