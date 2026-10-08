//! One connection's state, with no socket and no clock: whoever owns the socket hands in what
//! happened and the time it happened, and sends what comes back.
//!
//! - [`Session::subscribe`] and [`Session::unsubscribe`] keep the wanted set and give the frame
//!   to send while the socket is open. The set outlives the socket: [`Session::opened`] gives
//!   the frames that subscribe to all of it again, since a new socket starts with nothing.
//! - [`Session::received`] reads a frame and notes that something arrived.
//! - [`Session::tick`] pings a socket that has been quiet for 30 seconds, and says when one has
//!   been silent so long it is dead (hip4-tui's D8: the venue closes a connection idle for about
//!   a minute, and a book that silently stopped is worse than one that shows it is down).
//! - [`Session::closed`] says how long to wait before connecting again: doubling, bounded,
//!   jittered, and never giving up.
//!
//! The venue's per-IP limits (docs, "Rate limits and user limits", read 2026-10-08) are counted
//! here and refused before anything is sent: [`MAX_SUBSCRIPTIONS`], [`MAX_USERS`] and
//! [`MAX_INFLIGHT_POSTS`]. The rest are the socket owner's to keep: [`MAX_CONNECTIONS`],
//! [`MAX_NEW_CONNECTIONS_PER_MINUTE`] and [`MAX_MESSAGES_PER_MINUTE`]. The limits are per IP, so
//! an app with several sockets splits them ([`Session::with_limits`]).

use hl_core::Network;

use crate::{
    frame::{BadFrame, Frame, decode},
    request::{Post, post, subscribe, unsubscribe},
    subscription::{BadSubscription, Subscription},
};

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
    pub subscriptions: usize,
    pub users: usize,
    pub posts: usize,
}

impl Limits {
    pub const VENUE: Limits = Limits { subscriptions: MAX_SUBSCRIPTIONS, users: MAX_USERS, posts: MAX_INFLIGHT_POSTS };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::VENUE
    }
}

/// The clock's rules, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Ping a socket nothing has arrived on for this long. The venue closes one it has sent
    /// nothing to for 60 s; a ping makes it send a pong.
    pub ping_after_ms: u64,
    /// A socket nothing has arrived on for this long, pong included, is dead. The venue would
    /// have closed it by now; when no close came, the connection is half open.
    pub silent_ms: u64,
    /// A socket that stayed open this long and heard something worked: the next failure starts
    /// the reconnect delays over. Acknowledgements alone in its first moments prove nothing; a
    /// socket the venue drops right after them must not reconnect at once, forever.
    pub steady_ms: u64,
    /// The first reconnect delay; each failure after doubles it.
    pub first_retry_ms: u64,
    /// The longest reconnect delay, before jitter adds up to a quarter.
    pub last_retry_ms: u64,
}

impl Timing {
    /// A 30 s ping (hip4-tui's D8), dead after 60 s of nothing (the venue's own idle limit),
    /// and hip4-tui's reconnect ladder: 250 ms doubling to 20 s.
    pub const DEFAULT: Timing = Timing {
        ping_after_ms: 30_000,
        silent_ms: 60_000,
        steady_ms: 30_000,
        first_retry_ms: 250,
        last_retry_ms: 20_000,
    };
}

impl Default for Timing {
    fn default() -> Self {
        Timing::DEFAULT
    }
}

/// Why nothing was sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("{limit} subscriptions is the most this session holds; the venue allows an IP 1000")]
    Subscriptions { limit: usize },
    #[error("{limit} users is the most this session follows; the venue allows an IP 10")]
    Users { limit: usize },
    #[error("{limit} posts are already waiting for an answer; the venue allows an IP 100")]
    Posts { limit: usize },
    /// A post is never held to send later: an action sent late may land after the moment it
    /// was meant for, and one sent again may land twice.
    #[error("the socket is not open, and a post is never held to send later")]
    Closed,
    #[error(transparent)]
    Bad(#[from] BadSubscription),
}

/// What a tick asks of the socket's owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Wait,
    /// Send [`crate::request::PING`].
    Ping,
    /// Nothing has arrived for too long: close the socket, call [`Session::closed`], and
    /// connect again after the delay it gives.
    Reconnect,
}

/// What a closed socket leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
    /// Wait this long, then connect again.
    pub after_ms: u64,
    /// Posts sent and never answered. Unknown, not failed: an action among them may have
    /// landed, so none is sent again; read the account to find out.
    pub unanswered: Vec<u64>,
}

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

    /// What the socket needs at `now`: a ping after [`Timing::ping_after_ms`] of quiet (once,
    /// until something arrives or as long again passes), or a reconnect after
    /// [`Timing::silent_ms`] of nothing.
    pub fn tick(&mut self, now: u64) -> Tick {
        let Link::Open { heard, pinged, .. } = &mut self.link else { return Tick::Wait };
        if now.saturating_sub(*heard) >= self.timing.silent_ms {
            return Tick::Reconnect;
        }
        if now.saturating_sub((*heard).max(*pinged)) >= self.timing.ping_after_ms {
            *pinged = now;
            return Tick::Ping;
        }
        Tick::Wait
    }

    /// When a tick next has something to do; `None` while closed.
    #[must_use]
    pub fn due(&self) -> Option<u64> {
        let Link::Open { heard, pinged, .. } = self.link else { return None };
        let ping = heard.max(pinged).saturating_add(self.timing.ping_after_ms);
        let silent = heard.saturating_add(self.timing.silent_ms);
        Some(ping.min(silent))
    }

    /// The socket closed at `now`, or a connect failed. Gives how long to wait before the next
    /// connect, and the posts left unanswered. The wait doubles from
    /// [`Timing::first_retry_ms`] to [`Timing::last_retry_ms`] and `random` adds up to a quarter
    /// more, so many clients dropped at once do not return at once. It starts over after a
    /// socket that held ([`Timing::steady_ms`]). There is no last attempt.
    pub fn closed(&mut self, now: u64, random: u64) -> Retry {
        if let Link::Open { since, talked: true, .. } = self.link
            && now.saturating_sub(since) >= self.timing.steady_ms
        {
            self.failures = 0;
        }
        self.link = Link::Closed;
        let doubled = 1u64.checked_shl(self.failures).unwrap_or(u64::MAX);
        let step = self.timing.first_retry_ms.saturating_mul(doubled).min(self.timing.last_retry_ms);
        let jitter = random % (step / 4).saturating_add(1);
        self.failures = self.failures.saturating_add(1);
        Retry { after_ms: step.saturating_add(jitter), unanswered: std::mem::take(&mut self.waiting) }
    }
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;
