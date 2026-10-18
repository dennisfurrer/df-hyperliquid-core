//! Signatures as the venue reads them: made low-s with an Ethereum `v`, read from a wallet in
//! either form, recovered to their signer, and refused when malformed.

use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::*;
use crate::utils::sign::eip712::Field;
use crate::utils::sign::keccak::keccak256;

const ONE: [u8; 32] = {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    bytes
};
const ONE_ADDRESS: &str = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";

/// The secp256k1 group order, for making the high-s twin of a signature.
const ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc,
    0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

fn key(bytes: &[u8; 32]) -> LocalKey {
    LocalKey::from_bytes(bytes).unwrap()
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// `ORDER - s`, big-endian.
fn negate(s: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut borrow = 0i16;
    for at in (0..32).rev() {
        let mut digit = i16::from(ORDER[at]) - i16::from(s[at]) - borrow;
        borrow = 0;
        if digit < 0 {
            digit += 256;
            borrow = 1;
        }
        out[at] = digit as u8;
    }
    out
}

fn wallet_hex(r: &[u8; 32], s: &[u8; 32], v: u8) -> String {
    let mut bytes = [0u8; 65];
    bytes[..32].copy_from_slice(r);
    bytes[32..64].copy_from_slice(s);
    bytes[64] = v;
    hex::encode_prefixed(&bytes)
}

/// Published keys: secp256k1's generator, its double, and the vectors' key.
#[test]
fn the_published_keys_derive_the_published_addresses() {
    let mut two = [0u8; 32];
    two[31] = 2;
    let vectors = LocalKey::from_hex("0x0123456789012345678901234567890123456789012345678901234567890123").unwrap();
    for (key, address) in [
        (key(&ONE), ONE_ADDRESS),
        (key(&two), "0x2b5ad5c4795c026514f8317c7a215e218dccd6cf"),
        (vectors, "0x14791697260e4c9a71f18484c9f997b308e59325"),
    ] {
        assert_eq!(key.address().to_hex(), address);
    }
}

#[test]
fn a_signature_recovers_to_its_signer_and_verify_names_any_other() {
    let key = key(&ONE);
    let digest = keccak256(b"an action hash stands in for itself here");
    let signature = key.sign_digest(&digest).unwrap();
    assert_eq!(recover(&digest, &signature), Ok(key.address()));
    assert_eq!(verify(&digest, &signature, key.address()), Ok(()));
    let other = keccak256(b"a different action entirely");
    assert_ne!(recover(&other, &signature).ok(), Some(key.address()));
    let expected: Address = "0x2b5ad5c4795c026514f8317c7a215e218dccd6cf".parse().unwrap();
    assert_eq!(
        verify(&digest, &signature, expected),
        Err(SignError::WrongSigner { expected, recovered: ONE_ADDRESS.parse().unwrap() })
    );
}

/// `{"r":"0x..","s":"0x..","v":27}`, as `/exchange` reads it: `r` and `s` as full words.
#[test]
fn a_signature_is_low_s_carries_an_ethereum_v_and_posts_as_r_s_v() {
    let key = key(&ONE);
    for index in 0u8..16 {
        let digest = keccak256(&[index; 7]);
        let signature = key.sign_digest(&digest).unwrap();
        assert!(signature.v() == 27 || signature.v() == 28, "v was {}", signature.v());
        assert_eq!(signature.recovery_id(), Some(signature.v() - 27));
        let curve = CurveSignature::from_slice(&signature.to_compact()).unwrap();
        assert!(curve.normalize_s().is_none(), "signature {index} was not low-s normalised");
        assert_eq!(recover(&digest, &signature).unwrap(), key.address());
        let json = signature.to_json();
        assert_eq!(
            json,
            format!(r#"{{"r":"{}","s":"{}","v":{}}}"#, signature.r_hex(), signature.s_hex(), signature.v())
        );
        assert_eq!(serde_json::to_string(&signature).unwrap(), json, "serde writes the same");
        assert_eq!((signature.r_hex().len(), signature.s_hex().len()), (66, 66));
    }
}

/// `eth_signTypedData_v4` answers `r || s || v`, `v` as 0/1 or 27/28; a wallet that does not
/// normalise may send the high-s twin, folded here to the low one the venue takes.
#[test]
fn a_wallet_s_hex_reads_with_either_v_and_a_high_s_folded_to_its_low_twin() {
    let key = key(&ONE);
    for index in 0u8..8 {
        let digest = keccak256(&[index; 3]);
        let signed = key.sign_digest(&digest).unwrap();
        let parity = signed.v() - 27;
        let flipped = 27 + (parity ^ 1);
        for (s, v) in [(*signed.s(), parity), (*signed.s(), parity + 27), (negate(signed.s()), flipped)] {
            let read = Signature::from_hex(&wallet_hex(signed.r(), &s, v)).unwrap();
            assert_eq!(read, signed, "v given as {v}");
            assert_eq!(recover(&digest, &read), Ok(key.address()));
        }
    }
}

#[test]
fn a_malformed_signature_is_refused_rather_than_guessed_at() {
    let digest = keccak256(b"body");
    let signature = key(&ONE).sign_digest(&digest).unwrap();
    for (recovered, refused) in [
        (Signature::from_parts(*signature.r(), *signature.s(), 1), SignError::BadV(1)),
        (Signature::from_parts([0u8; 32], [0u8; 32], 27), SignError::Malformed),
    ] {
        assert_eq!(recover(&digest, &recovered), Err(refused));
    }
    for (hex, refused) in [
        ("0x1234".to_owned(), SignError::NotASignature),
        (wallet_hex(signature.r(), signature.s(), 29), SignError::BadV(29)),
        (wallet_hex(&[0; 32], &[0; 32], 27), SignError::Malformed),
    ] {
        assert_eq!(Signature::from_hex(&hex), Err(refused), "{hex}");
    }
    assert_eq!(LocalKey::from_hex("0xnot-a-key").unwrap_err(), SignError::NotAKey);
    assert_eq!(LocalKey::from_bytes(&[0u8; 32]).unwrap_err(), SignError::NotAKey);
}

#[test]
fn a_local_key_signs_typed_data_as_the_trait_and_never_shows_itself() {
    let key = LocalKey::from_hex("0x0123456789012345678901234567890123456789012345678901234567890123").unwrap();
    let data =
        TypedStruct::new("Agent").field("source", Field::String("a")).field("connectionId", Field::Bytes32([7; 32]));
    let signed = block_on(key.sign_typed(&Domain::EXCHANGE, &data)).unwrap();
    assert_eq!(recover(&data.digest(&Domain::EXCHANGE), &signed), Ok(Signer::address(&key)));
    let shown = format!("{key:?}");
    assert!(shown.contains("0x14791697260e4c9a71f18484c9f997b308e59325"), "{shown}");
    assert!(!shown.contains("0123456789012345678901234567890123456789012345678901234567890123"), "{shown}");
}
