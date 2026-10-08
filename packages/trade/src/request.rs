//! An `/info` body: `type` first, then the fields in the order the venue's
//! docs list them, so a request reads the same wherever it is logged.

use hl_core::{Address, Json};

/// `{"type":kind, ...fields}`.
pub(crate) fn body(kind: &str, fields: impl IntoIterator<Item = (&'static str, Json)>) -> String {
    let mut entries = vec![("type".to_owned(), Json::String(kind.to_owned()))];
    entries.extend(fields.into_iter().map(|(key, value)| (key.to_owned(), value)));
    Json::Object(entries).compact()
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
