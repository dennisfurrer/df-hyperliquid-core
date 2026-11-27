//! k256 and libsecp256k1 sign every digest to the same bytes and give the same answer for every
//! signature recovered: valid ones, high-s twins, every bad `v`, `r` or `s` at zero or the group
//! order, and random bytes.

use super::*;
use crate::account::LocalKey;

/// Deterministic bytes: a 64-bit LCG, so a failure names a seed that reproduces it.
struct Bytes(u64);

impl Bytes {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn fill<const N: usize>(&mut self) -> [u8; N] {
        let mut bytes = [0u8; N];
        for chunk in bytes.chunks_mut(8) {
            let word = self.next().to_be_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
        bytes
    }
}

/// `ORDER - s`: the high-s twin of a low `s`.
fn negate(s: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut borrow = 0i16;
    for at in (0..32).rev() {
        let mut digit = i16::from(ORDER[at]) - i16::from(s[at]) - borrow;
        borrow = i16::from(digit < 0);
        if digit < 0 {
            digit += 256;
        }
        out[at] = digit as u8;
    }
    out
}

fn both(digest: &[u8; 32], signature: &Signature) -> (Result<Address, SignError>, Result<Address, SignError>) {
    (k256_backend::recover(digest, signature), libsecp_backend::recover(digest, signature))
}

#[test]
fn both_backends_sign_to_the_same_bytes_and_recover_the_signer() {
    let mut bytes = Bytes(7);
    for round in 0..300 {
        let Ok(key) = LocalKey::from_bytes(&bytes.fill()) else { continue };
        let digest: [u8; 32] = bytes.fill();
        let curve = key.curve_key();
        let pure = k256_backend::Key::new(curve).unwrap().sign(curve, &digest).unwrap();
        let fast = libsecp_backend::Key::new(curve).unwrap().sign(curve, &digest).unwrap();
        assert_eq!(pure, fast, "round {round}");
        assert_eq!(key.sign_digest(&digest).unwrap(), fast, "round {round}");
        let (pure, fast) = both(&digest, &fast);
        assert_eq!(pure, Ok(key.address()), "round {round}");
        assert_eq!(fast, Ok(key.address()), "round {round}");
    }
}

/// RFC 6979 reduces a digest at or above the group order before deriving the nonce, and
/// libsecp256k1 does not; such digests still sign to the same bytes. Found by the fuzz target.
#[test]
fn a_digest_at_or_above_the_group_order_signs_the_same() {
    let mut above = ORDER;
    above[31] += 1;
    let key = LocalKey::from_bytes(&[1; 32]).unwrap();
    let curve = key.curve_key();
    for digest in
        [ORDER, above, [0xff; 32], [0xff; 18].iter().chain(&[1; 14]).copied().collect::<Vec<u8>>().try_into().unwrap()]
    {
        let pure = k256_backend::Key::new(curve).unwrap().sign(curve, &digest).unwrap();
        let fast = libsecp_backend::Key::new(curve).unwrap().sign(curve, &digest).unwrap();
        assert_eq!(pure, fast, "{digest:x?}");
        assert_eq!(libsecp_backend::recover(&digest, &fast), Ok(key.address()));
    }
}

#[test]
fn both_backends_refuse_the_same_signatures() {
    let mut bytes = Bytes(11);
    let key = LocalKey::from_bytes(&bytes.fill()).unwrap();
    let digest: [u8; 32] = bytes.fill();
    let good = key.sign_digest(&digest).unwrap();
    let compact = good.to_compact();
    let (r, s): ([u8; 32], [u8; 32]) = (compact[..32].try_into().unwrap(), compact[32..].try_into().unwrap());

    let mut cases = vec![
        // The high-s twin, with either `v`: k256 refuses to recover a high `s`.
        Signature::from_parts(r, negate(&s), good.v()),
        Signature::from_parts(r, negate(&s), good.v() ^ 1),
        Signature::from_parts([0; 32], s, good.v()),
        Signature::from_parts(r, [0; 32], good.v()),
        Signature::from_parts(ORDER, s, good.v()),
        Signature::from_parts(r, ORDER, good.v()),
        Signature::from_parts([0xff; 32], s, good.v()),
        Signature::from_parts(r, s, good.v() ^ 1),
    ];
    cases.extend((0..=u8::MAX).map(|v| Signature::from_parts(r, s, v)));
    for _ in 0..2_000 {
        cases.push(Signature::from_parts(bytes.fill(), bytes.fill(), 27 + (bytes.next() % 2) as u8));
    }
    let mut refused = 0;
    for signature in &cases {
        let (pure, fast) = both(&digest, signature);
        assert_eq!(pure, fast, "{signature:?}");
        refused += usize::from(pure.is_err());
    }
    assert!(refused > 250, "only {refused} refused");
    assert_eq!(both(&digest, &cases[0]).0, Err(SignError::Unrecoverable));
}
