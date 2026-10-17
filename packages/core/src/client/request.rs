//! An `/info` body: `type` first, then the fields in the order the venue's
//! docs list them, so a request reads the same wherever it is logged.

use crate::client::info_request;
use crate::utils::{Address, Json};

/// `{"type":kind, ...fields}`.
pub(crate) fn body(kind: &str, fields: impl IntoIterator<Item = (&'static str, Json)>) -> String {
    info_request(kind, &fields.into_iter().collect::<Vec<_>>())
}

pub(crate) fn text(value: &str) -> Json {
    Json::String(value.to_owned())
}

pub(crate) fn user(user: Address) -> (&'static str, Json) {
    ("user", Json::String(user.to_hex()))
}

pub(crate) fn number(value: u64) -> Json {
    Json::Number(value.to_string())
}

/// `dex` when one is named; the main dex when not.
pub(crate) fn dex(dex: Option<&str>) -> Option<(&'static str, Json)> {
    dex.map(|dex| ("dex", text(dex)))
}
