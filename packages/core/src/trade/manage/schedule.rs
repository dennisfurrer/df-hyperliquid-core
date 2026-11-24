//! What stops orders without naming them: `scheduleCancel`, the dead man's
//! switch, and `noop`, which spends a nonce so an order signed with it never
//! lands.

use crate::utils::sign::{L1Action, Map};

/// `scheduleCancel`: at `time` (unix milliseconds) the venue cancels every open order of the
/// account. Armed with a time, disarmed without one: the key is left out, not written `null`. The
/// time is at least five seconds ahead, and the switch fires at most ten times a day, counted
/// from 00:00 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleCancel {
    time: Option<u64>,
}

/// A scheduled cancel the venue would refuse: under five seconds away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a scheduled cancel at {time} is under five seconds away; the earliest is {earliest}")]
pub struct TooSoon {
    /// The time asked for, unix milliseconds.
    pub time: u64,
    /// The earliest time the venue would take, unix milliseconds.
    pub earliest: u64,
}

impl ScheduleCancel {
    /// The least a switch may be armed ahead of now.
    pub const LEAD_MS: u64 = 5_000;

    /// How many times a day the switch may fire.
    pub const DAILY_TRIGGERS: u32 = 10;

    /// Armed for `time`, checked against `now` (both unix milliseconds): the
    /// clock is the caller's.
    pub const fn arm(time: u64, now: u64) -> Result<ScheduleCancel, TooSoon> {
        let earliest = now.saturating_add(ScheduleCancel::LEAD_MS);
        match time >= earliest {
            true => Ok(ScheduleCancel { time: Some(time) }),
            false => Err(TooSoon { time, earliest }),
        }
    }

    /// Armed for `time`, unchecked.
    #[must_use]
    pub const fn at(time: u64) -> ScheduleCancel {
        ScheduleCancel { time: Some(time) }
    }

    /// Takes a switch that was armed away.
    #[must_use]
    pub const fn disarm() -> ScheduleCancel {
        ScheduleCancel { time: None }
    }

    /// When the switch fires, unix milliseconds; `None` when it is disarmed.
    #[must_use]
    pub const fn time(&self) -> Option<u64> {
        self.time
    }
}

impl L1Action for ScheduleCancel {
    fn action(&self) -> Map {
        Map::new().put("type", "scheduleCancel").put_some("time", self.time)
    }
}

/// `noop`: does nothing but mark its nonce used. The venue's exchange docs
/// call it a more effective way to cancel an order still in flight than a
/// cancel: sign a noop with that order's nonce and the order cannot land.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Noop;

impl L1Action for Noop {
    fn action(&self) -> Map {
        Map::new().put("type", "noop")
    }
}

#[cfg(test)]
#[path = "tests/schedule.rs"]
mod tests;
