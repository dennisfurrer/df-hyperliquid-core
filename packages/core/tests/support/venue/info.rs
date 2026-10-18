//! `/info`: each read answered from the recording given for its `type`, and its fields when
//! asked.

use df_hyperliquid_core::utils::Json;

use super::{Answer, Sent, Venue, ok};
use crate::support::fixture;

/// One `/info` answer: for reads of `kind` whose fields match.
pub(super) struct Read {
    kind: String,
    fields: Vec<(String, String)>,
    answer: String,
}

impl Venue {
    /// Answers every `{"type":kind}` read with the recording at `fixtures/<path>`.
    #[must_use]
    pub fn info(self, kind: &str, path: &str) -> Venue {
        self.info_text(kind, &fixture(path))
    }

    /// Answers every `{"type":kind}` read with `answer`.
    #[must_use]
    pub fn info_text(self, kind: &str, answer: &str) -> Venue {
        self.info_where(kind, &[], answer)
    }

    /// Answers `{"type":kind}` reads whose fields hold these values (strings as written, other
    /// values as compact JSON) with `answer`. The first match wins, so give narrow reads first.
    #[must_use]
    pub fn info_where(mut self, kind: &str, fields: &[(&str, &str)], answer: &str) -> Venue {
        let fields = fields.iter().map(|(key, value)| ((*key).to_owned(), (*value).to_owned())).collect();
        self.reads.push(Read { kind: kind.to_owned(), fields, answer: answer.to_owned() });
        self
    }

    pub(super) fn read(&self, body: &str, failure: Option<Answer>) -> Answer {
        let read = Json::parse(body).unwrap_or_else(|error| panic!("an /info body that is not JSON ({error}): {body}"));
        let kind =
            read.get("type").and_then(Json::as_str).unwrap_or_else(|| panic!("an /info read with no type: {body}"));
        self.sent.borrow_mut().push(Sent::Info(read.clone()));
        failure.unwrap_or_else(|| {
            let route = self.reads.iter().find(|route| {
                route.kind == kind && route.fields.iter().all(|(key, value)| matches(read.get(key), value))
            });
            ok(route.unwrap_or_else(|| panic!("the venue has no answer for {body}")).answer.clone())
        })
    }
}

fn matches(value: Option<&Json>, expected: &str) -> bool {
    match value {
        Some(Json::String(text)) => text.eq_ignore_ascii_case(expected),
        Some(other) => other.compact() == expected,
        None => false,
    }
}
