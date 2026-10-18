//! A post's answer, by the id it was sent with: what `/info` or `/exchange` would have answered,
//! or what HTTP would have said when the post was not taken.

use crate::client::ExchangeResponse;

/// The answer to a post, by the id it was sent with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: u64,
    pub answer: Answer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// What `/info` would have answered, as written (`null` for an unknown coin's book), for a
    /// read's own type to deserialize.
    Info { kind: String, data: String },
    /// What `/exchange` would have answered.
    Action(ExchangeResponse),
    /// The post was not taken: what HTTP would have said.
    Error(String),
}

#[cfg(test)]
#[path = "tests/reply.rs"]
mod tests;
