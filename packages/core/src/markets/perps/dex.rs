//! Request bodies for perp reads that take a dex: no `dex` for the main dex, `"dex":"xyz"` for a
//! HIP-3 dex.

use crate::client::info_request;
use crate::utils::Json;

/// The main dex or a HIP-3 dex by name.
pub use crate::markets::Dex;

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
