use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::*;
use crate::{eip712::Field, keccak::keccak256};

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

#[test]
fn the_published_key_derives_the_published_address() {
    assert_eq!(key(&ONE).address().to_hex(), ONE_ADDRESS);
    let mut two = [0u8; 32];
    two[31] = 2;
    assert_eq!(key(&two).address().to_hex(), "0x2b5ad5c4795c026514f8317c7a215e218dccd6cf");
}

#[test]
fn a_signature_recovers_to_its_own_signer() {
    let key = key(&ONE);
    let digest = keccak256(b"an action hash stands in for itself here");
    let signature = key.sign_digest(&digest).unwrap();
    assert_eq!(recover(&digest, &signature).unwrap(), key.address());
    assert_eq!(verify(&digest, &signature, key.address()), Ok(()));
    let other = keccak256(b"a different action entirely");
    assert_ne!(recover(&other, &signature).ok(), Some(key.address()));
}

#[test]
fn a_signature_from_another_account_is_named_and_refused() {
    let digest = keccak256(b"body");
    let signature = key(&ONE).sign_digest(&digest).unwrap();
    let expected: Address = "0x2b5ad5c4795c026514f8317c7a215e218dccd6cf".parse().unwrap();
    assert_eq!(
        verify(&digest, &signature, expected),
        Err(SignError::WrongSigner { expected, recovered: ONE_ADDRESS.parse().unwrap() })
    );
}

#[test]
fn signatures_are_low_s_and_carry_an_ethereum_v() {
    let key = key(&ONE);
    for index in 0u8..16 {
        let digest = keccak256(&[index; 7]);
        let signature = key.sign_digest(&digest).unwrap();
        assert!(signature.v() == 27 || signature.v() == 28, "v was {}", signature.v());
        assert_eq!(signature.recovery_id(), Some(signature.v() - 27));
        let curve = CurveSignature::from_slice(&signature.to_compact()).unwrap();
        assert!(curve.normalize_s().is_none(), "signature {index} was not low-s normalised");
        assert_eq!(recover(&digest, &signature).unwrap(), key.address());
    }
}

#[test]
fn the_json_shape_is_the_one_the_venue_reads() {
    let signature = key(&ONE).sign_digest(&keccak256(b"body")).unwrap();
    let json = signature.to_json();
    assert_eq!(json, format!(r#"{{"r":"{}","s":"{}","v":{}}}"#, signature.r_hex(), signature.s_hex(), signature.v()));
    assert_eq!(serde_json::to_string(&signature).unwrap(), json, "serde writes the same");
    assert_eq!(signature.r_hex().len(), 66);
    assert_eq!(signature.s_hex().len(), 66);
}

#[test]
fn a_wallet_s_hex_reads_with_either_v_and_comes_out_27_or_28() {
    let key = key(&ONE);
    for index in 0u8..8 {
        let digest = keccak256(&[index; 3]);
        let signed = key.sign_digest(&digest).unwrap();
        let parity = signed.v() - 27;
        for v in [parity, parity + 27] {
            let read = Signature::from_hex(&wallet_hex(signed.r(), signed.s(), v)).unwrap();
            assert_eq!(read, signed, "v given as {v}");
            assert_eq!(recover(&digest, &read), Ok(key.address()));
        }
    }
}

#[test]
fn a_high_s_from_a_wallet_is_folded_to_its_low_twin() {
    let key = key(&ONE);
    let digest = keccak256(b"a wallet that does not normalise");
    let signed = key.sign_digest(&digest).unwrap();
    let high = negate(signed.s());
    let flipped = if signed.v() == 27 { 28 } else { 27 };
    let read = Signature::from_hex(&wallet_hex(signed.r(), &high, flipped)).unwrap();
    assert_eq!(read, signed);
    assert_eq!(recover(&digest, &read), Ok(key.address()));
}

#[test]
fn a_malformed_signature_is_refused_rather_than_guessed_at() {
    let digest = keccak256(b"body");
    let signature = key(&ONE).sign_digest(&digest).unwrap();
    let wrong_v = Signature::from_parts(*signature.r(), *signature.s(), 1);
    assert_eq!(recover(&digest, &wrong_v), Err(SignError::BadV(1)));
    let zeroed = Signature::from_parts([0u8; 32], [0u8; 32], 27);
    assert_eq!(recover(&digest, &zeroed), Err(SignError::Malformed));
    assert_eq!(Signature::from_hex("0x1234"), Err(SignError::NotASignature));
    assert_eq!(Signature::from_hex(&wallet_hex(signature.r(), signature.s(), 29)), Err(SignError::BadV(29)));
    assert_eq!(Signature::from_hex(&wallet_hex(&[0; 32], &[0; 32], 27)), Err(SignError::Malformed));
}

#[test]
fn a_local_key_signs_typed_data_as_the_trait_and_never_shows_itself() {
    let key = LocalKey::from_hex("0x0123456789012345678901234567890123456789012345678901234567890123").unwrap();
    assert_eq!(key.address().to_hex(), "0x14791697260e4c9a71f18484c9f997b308e59325");
    let data =
        TypedStruct::new("Agent").field("source", Field::String("a")).field("connectionId", Field::Bytes32([7; 32]));
    let signed = block_on(key.sign_typed(&Domain::EXCHANGE, &data)).unwrap();
    assert_eq!(recover(&data.digest(&Domain::EXCHANGE), &signed), Ok(Signer::address(&key)));
    let shown = format!("{key:?}");
    assert!(shown.contains("0x14791697260e4c9a71f18484c9f997b308e59325"), "{shown}");
    assert!(!shown.contains("0123456789012345678901234567890123456789012345678901234567890123"), "{shown}");
    assert_eq!(LocalKey::from_hex("0xnot-a-key").unwrap_err(), SignError::NotAKey);
    assert_eq!(LocalKey::from_bytes(&[0u8; 32]).unwrap_err(), SignError::NotAKey);
}
