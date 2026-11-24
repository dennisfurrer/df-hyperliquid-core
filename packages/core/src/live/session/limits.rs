//! The venue's per-IP limits, the part of them one session holds ([`Limits`]), and why a session
//! sends nothing ([`Refused`]).

use crate::live::BadSubscription;

/// Websocket connections an IP may hold.
pub const MAX_CONNECTIONS: usize = 10;
/// New websocket connections an IP may open a minute.
pub const MAX_NEW_CONNECTIONS_PER_MINUTE: usize = 30;
/// Subscriptions an IP may hold across its connections.
pub const MAX_SUBSCRIPTIONS: usize = 1_000;
/// Distinct users an IP's user subscriptions may name.
pub const MAX_USERS: usize = 10;
/// Messages an IP may send a minute across its connections.
pub const MAX_MESSAGES_PER_MINUTE: usize = 2_000;
/// Posts an IP may have waiting for an answer across its connections.
pub const MAX_INFLIGHT_POSTS: usize = 100;

/// What one session may hold. [`Limits::VENUE`] is the whole of an IP's allowance; a session
/// sharing the IP with others takes a part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Subscriptions the session may hold.
    pub subscriptions: usize,
    /// Distinct users the session's subscriptions may name.
    pub users: usize,
    /// Posts the session may have waiting for an answer.
    pub posts: usize,
}

impl Limits {
    /// An IP's whole allowance: 1000 subscriptions, 10 users, 100 posts in flight.
    pub const VENUE: Limits = Limits { subscriptions: MAX_SUBSCRIPTIONS, users: MAX_USERS, posts: MAX_INFLIGHT_POSTS };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::VENUE
    }
}

/// Why nothing was sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The session already holds as many subscriptions as its limits allow.
    #[error("{limit} subscriptions is the most this session holds; the venue allows an IP 1000")]
    Subscriptions {
        /// The session's limit, [`Limits::subscriptions`].
        limit: usize,
    },
    /// The subscription would name one user more than the session's limits allow.
    #[error("{limit} users is the most this session follows; the venue allows an IP 10")]
    Users {
        /// The session's limit, [`Limits::users`].
        limit: usize,
    },
    /// As many posts as the session's limits allow are waiting for an answer.
    #[error("{limit} posts are already waiting for an answer; the venue allows an IP 100")]
    Posts {
        /// The session's limit, [`Limits::posts`].
        limit: usize,
    },
    /// A post is never held to send later: an action sent late may land after the moment it
    /// was meant for, and one sent again may land twice.
    #[error("the socket is not open, and a post is never held to send later")]
    Closed,
    /// The subscription is not one the venue would take.
    #[error(transparent)]
    Bad(#[from] BadSubscription),
}

#[cfg(test)]
#[path = "tests/limits.rs"]
mod tests;
