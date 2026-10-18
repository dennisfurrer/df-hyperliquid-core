//! One connection's state with no socket and no clock: the socket's owner hands in what happened
//! and when, and sends what comes back. Of the venue's per-IP limits, subscriptions, users and
//! posts in flight are refused here; connections and message rates are the owner's to keep.

pub mod limits;
pub mod timing;

pub use limits::{
    Limits, MAX_CONNECTIONS, MAX_INFLIGHT_POSTS, MAX_MESSAGES_PER_MINUTE, MAX_NEW_CONNECTIONS_PER_MINUTE,
    MAX_SUBSCRIPTIONS, MAX_USERS, Refused,
};
pub use timing::{Retry, Tick, Timing};

use crate::client::Network;
use crate::live::{BadFrame, Frame, Post, Subscription, decode, post, subscribe, unsubscribe};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    Closed,
    Open {
        /// When it opened.
        since: u64,
        /// When anything last arrived; `since` until something does.
        heard: u64,
        /// Whether anything has arrived at all.
        talked: bool,
        /// When the last ping went out; `since` until one does.
        pinged: u64,
    },
}

/// One connection to one network's websocket.
#[derive(Debug, Clone)]
pub struct Session {
    network: Network,
    limits: Limits,
    timing: Timing,
    wanted: Vec<Subscription>,
    link: Link,
    /// Reconnects since a socket last worked.
    failures: u32,
    next_id: u64,
    waiting: Vec<u64>,
}

impl Session {
    /// A closed session on `network`, with the venue's limits and the default timing.
    #[must_use]
    pub fn new(network: Network) -> Self {
        Session {
            network,
            limits: Limits::VENUE,
            timing: Timing::DEFAULT,
            wanted: Vec::new(),
            link: Link::Closed,
            failures: 0,
            next_id: 1,
            waiting: Vec::new(),
        }
    }

    /// The limits are per IP: sessions sharing one split them.
    #[must_use]
    pub const fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub const fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }

    /// Where to connect: the network's public websocket.
    #[must_use]
    pub const fn url(&self) -> &'static str {
        self.network.ws_url()
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self.link, Link::Open { .. })
    }

    /// What this session subscribes to on every socket, in the order it was asked for.
    #[must_use]
    pub fn wanted(&self) -> &[Subscription] {
        &self.wanted
    }

    /// The distinct users the wanted subscriptions follow.
    #[must_use]
    pub fn users(&self) -> usize {
        let mut users: Vec<_> = self.wanted.iter().filter_map(Subscription::user).collect();
        users.sort_unstable();
        users.dedup();
        users.len()
    }

    /// The ids of posts sent on this socket and not yet answered.
    #[must_use]
    pub fn waiting(&self) -> &[u64] {
        &self.waiting
    }

    /// Wants `subscription` from now on, on this socket and every later one. Gives the frame to
    /// send when the socket is open and it is new; nothing when it was already wanted. Past the
    /// limits, or when the venue would not take it, it is refused and not kept.
    pub fn subscribe(&mut self, subscription: Subscription) -> Result<Option<String>, Refused> {
        if self.wanted.contains(&subscription) {
            return Ok(None);
        }
        subscription.check()?;
        if self.wanted.len() >= self.limits.subscriptions {
            return Err(Refused::Subscriptions { limit: self.limits.subscriptions });
        }
        if let Some(user) = subscription.user()
            && !self.wanted.iter().any(|wanted| wanted.user() == Some(user))
            && self.users() >= self.limits.users
        {
            return Err(Refused::Users { limit: self.limits.users });
        }
        let frame = self.is_open().then(|| subscribe(&subscription));
        self.wanted.push(subscription);
        Ok(frame)
    }

    /// Stops wanting `subscription`. Gives the frame to send when the socket is open and it was
    /// wanted.
    pub fn unsubscribe(&mut self, subscription: &Subscription) -> Option<String> {
        let at = self.wanted.iter().position(|wanted| wanted == subscription)?;
        let gone = self.wanted.remove(at);
        self.is_open().then(|| unsubscribe(&gone))
    }

    /// Gives the next post's id and its frame. Only on an open socket, and only while fewer
    /// than the limit are waiting for an answer.
    pub fn post(&mut self, request: &Post) -> Result<(u64, String), Refused> {
        if !self.is_open() {
            return Err(Refused::Closed);
        }
        if self.waiting.len() >= self.limits.posts {
            return Err(Refused::Posts { limit: self.limits.posts });
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.waiting.push(id);
        Ok((id, post(id, request)))
    }

    /// The socket opened at `now`. Gives the frames that subscribe to everything wanted, in
    /// order.
    pub fn opened(&mut self, now: u64) -> Vec<String> {
        self.link = Link::Open { since: now, heard: now, talked: false, pinged: now };
        self.wanted.iter().map(subscribe).collect()
    }

    /// A text frame arrived at `now`. Anything that arrives shows the socket is alive, whether it
    /// reads or not. A post's answer or refusal stops it waiting.
    pub fn received(&mut self, now: u64, text: &str) -> Result<Frame, BadFrame> {
        if let Link::Open { heard, talked, .. } = &mut self.link {
            *heard = (*heard).max(now);
            *talked = true;
        }
        let frame = decode(text);
        let answered = match &frame {
            Ok(Frame::Post(reply)) => Some(reply.id),
            Ok(Frame::Error(refusal)) => refusal.post_id(),
            _ => None,
        };
        if let Some(id) = answered {
            self.waiting.retain(|waiting| *waiting != id);
        }
        frame
    }
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
