//! Which perp dex a read is about. The venue keeps one clearinghouse per dex
//! and answers for one at a time: a request without `dex` is the main dex's,
//! one with `"dex":"xyz"` that HIP-3 dex's.

use hl_core::{Json, info_request};

/// The main dex or a HIP-3 dex by name: hl-core's, shared with hl-hip3.
pub use hl_core::Dex;

/// `{"type":<kind>, ..fields, "dex":<name>}`, `dex` left out for the main dex.
pub(crate) fn request(kind: &str, mut fields: Vec<(&str, Json)>, dex: Dex<'_>) -> String {
    if let Some(name) = dex.name() {
        fields.push(("dex", Json::String(name.to_owned())));
    }
    info_request(kind, &fields)
}

/// A count or a time as the venue writes it: a bare integer.
pub(crate) fn number(value: u64) -> Json {
    Json::Number(value.to_string())
}

pub(crate) fn text(value: &str) -> Json {
    Json::String(value.to_owned())
}
