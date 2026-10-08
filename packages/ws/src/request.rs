//! The frames sent: subscribe, unsubscribe, ping, and a post, which carries an info read or a
//! signed action over the socket instead of over HTTP. Each is one text frame, as
//! `JSON.stringify` would write it, keys in the docs' order.

use hl_core::{Json, json::BadJson};

use crate::subscription::Subscription;

/// The heartbeat. The venue answers `{"channel":"pong"}`.
pub const PING: &str = r#"{"method":"ping"}"#;

/// `{"method":"subscribe","subscription":{...}}`.
#[must_use]
pub fn subscribe(subscription: &Subscription) -> String {
    wrap("subscribe", subscription)
}

/// `{"method":"unsubscribe","subscription":{...}}`, with the same object the subscribe sent:
/// the venue matches the options too.
#[must_use]
pub fn unsubscribe(subscription: &Subscription) -> String {
    wrap("unsubscribe", subscription)
}

fn wrap(method: &str, subscription: &Subscription) -> String {
    Json::Object(vec![
        ("method".to_owned(), Json::String(method.to_owned())),
        ("subscription".to_owned(), subscription.to_json()),
    ])
    .compact()
}

/// What a post carries: an info read, as it would be posted to `/info`, or a signed action, the
/// whole `/exchange` body. Either is kept as written, key order and digits, since an action's
/// signature is over exactly what was built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Post {
    Info(Json),
    Action(Json),
}

impl Post {
    /// An info read, the body `/info` would take: `{"type":"l2Book","coin":"ETH"}`.
    pub fn info(request: &str) -> Result<Post, BadJson> {
        Json::parse(request).map(Post::Info)
    }

    /// A signed action, the body `/exchange` would take: `{"action":..,"nonce":..,
    /// "signature":..,"vaultAddress":..}`. Like `/exchange`, never sent twice: a post that
    /// goes unanswered may still have landed.
    pub fn action(body: &str) -> Result<Post, BadJson> {
        Json::parse(body).map(Post::Action)
    }

    /// `info` or `action`, as the request's `type` names it.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Post::Info(_) => "info",
            Post::Action(_) => "action",
        }
    }

    #[must_use]
    pub const fn payload(&self) -> &Json {
        match self {
            Post::Info(payload) | Post::Action(payload) => payload,
        }
    }
}

/// `{"method":"post","id":..,"request":{"type":"info"|"action","payload":..}}`. The answer comes
/// on the channel `post` with the same `id`.
#[must_use]
pub fn post(id: u64, request: &Post) -> String {
    Json::Object(vec![
        ("method".to_owned(), Json::String("post".to_owned())),
        ("id".to_owned(), Json::Number(id.to_string())),
        (
            "request".to_owned(),
            Json::Object(vec![
                ("type".to_owned(), Json::String(request.kind().to_owned())),
                ("payload".to_owned(), request.payload().clone()),
            ]),
        ),
    ])
    .compact()
}

#[cfg(test)]
#[path = "tests/request.rs"]
mod tests;
