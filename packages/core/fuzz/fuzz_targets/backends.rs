//! Any key, digest and 65-byte signature: k256 and libsecp256k1 sign to the same bytes and
//! recover the same answer.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: ([u8; 32], [u8; 32], [u8; 32], [u8; 32], u8)| {
    let (secret, digest, r, s, v) = input;
    let mut signature = [0u8; 65];
    signature[..32].copy_from_slice(&r);
    signature[32..64].copy_from_slice(&s);
    signature[64] = v;
    df_hyperliquid_core::fuzz::backends_agree(&secret, &digest, &signature);
});
