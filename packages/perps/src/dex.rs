//! Which perp dex a read is about. The venue keeps one clearinghouse per dex
//! and answers for one at a time: a request without `dex` is the main dex's,
//! one with `"dex":"xyz"` that HIP-3 dex's.

use hl_core::{Json, info_request};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Dex<'a> {
    /// The main dex, the first in `perpDexs`.
    #[default]
    Main,
    /// A HIP-3 dex by its name: `xyz`, the `xyz:` of its coins.
    Hip3(&'a str),
}

impl<'a> Dex<'a> {
    /// The dex a coin trades on: `xyz:TSLA` on `xyz`, `BTC` on the main dex.
    #[must_use]
    pub fn of_coin(coin: &'a str) -> Dex<'a> {
        match coin.split_once(':') {
            Some((dex, _)) => Dex::Hip3(dex),
            None => Dex::Main,
        }
    }

    /// What a request names as its `dex`; nothing for the main dex.
    #[must_use]
    pub const fn name(self) -> Option<&'a str> {
        match self {
            Dex::Main => None,
            Dex::Hip3(name) => Some(name),
        }
    }
}

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
