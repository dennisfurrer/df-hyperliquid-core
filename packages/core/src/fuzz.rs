//! For the fuzz targets in `fuzz/` only: built when cargo-fuzz passes `--cfg fuzzing`, never part
//! of the crate's API. Each function panics when two readers or two backends that must agree do
//! not.

use crate::live::decode;
use crate::live::frame::one_pass::decode_in_one;
use crate::live::frame::read::decode_in_two;
use crate::utils::dec::Dec;

/// [`decode`] gives exactly what the two-pass reader gives, the same frame or the same error, and
/// the one-pass reader never reads a frame the two-pass reader reads differently.
pub fn decode_agrees(text: &str) {
    let two = decode_in_two(text);
    if let Ok(one) = decode_in_one(text) {
        assert_eq!(Ok(one), two, "one pass: {text}");
    }
    assert_eq!(decode(text), two, "decode: {text}");
}

/// The short decimal reader takes only what the long one reads, with the same `units` and
/// `scale`, and [`Dec::parse`] answers as the long one does.
pub fn dec_agrees(text: &str) {
    let long = Dec::parse_long(text).map(Dec::parts);
    if let Some(short) = Dec::parse_short(text.as_bytes()) {
        assert_eq!(Ok(short.parts()), long, "short: {text:?}");
    }
    assert_eq!(Dec::parse(text).map(Dec::parts), long, "parse: {text:?}");
}

/// k256 and libsecp256k1 sign `digest` with `secret` to the same bytes, and recover the same
/// answer from that signature and from `signature` (65 bytes, `r || s || v`), valid or not.
#[cfg(all(feature = "secp256k1", not(target_arch = "wasm32")))]
pub fn backends_agree(secret: &[u8; 32], digest: &[u8; 32], signature: &[u8; 65]) {
    use crate::account::keys::curve::{k256_backend, libsecp_backend};
    use crate::account::{LocalKey, Signature};

    if let Ok(key) = LocalKey::from_bytes(secret) {
        let curve = key.curve_key();
        let pure = k256_backend::Key::new(curve).and_then(|signer| signer.sign(curve, digest));
        let fast = libsecp_backend::Key::new(curve).and_then(|signer| signer.sign(curve, digest));
        assert_eq!(pure, fast, "sign");
        if let Ok(signed) = fast {
            assert_eq!(libsecp_backend::recover(digest, &signed), Ok(key.address()), "recover own");
            assert_eq!(k256_backend::recover(digest, &signed), Ok(key.address()), "recover own");
        }
    }
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&signature[..32]);
    s.copy_from_slice(&signature[32..64]);
    let any = Signature::from_parts(r, s, signature[64]);
    assert_eq!(k256_backend::recover(digest, &any), libsecp_backend::recover(digest, &any), "recover {any:?}");
}
