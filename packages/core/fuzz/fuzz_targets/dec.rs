//! Any text: the short decimal reader agrees with the long one on `units` and `scale`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        df_hyperliquid_core::fuzz::dec_agrees(text);
    }
});
