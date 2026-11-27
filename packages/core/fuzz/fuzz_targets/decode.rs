//! Any text: `live::decode` gives exactly what the two-pass reader gives, the same frame or the
//! same error, so the direct and one-pass readers never read a frame differently.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        df_hyperliquid_core::fuzz::decode_agrees(text);
    }
});
