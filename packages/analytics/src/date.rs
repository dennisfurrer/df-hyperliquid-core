//! Instants and days in UTC, counted from 1970 in integers on the proleptic Gregorian calendar:
//! hl-core's calendar. There is no clock here: whoever asks says when now is. A board's window,
//! a builder's day file, a projection's periods and a payout ledger's lines all name days or
//! instants, and every one of them is worked out there.

pub use hl_core::time::{At, BadAt, DAY_MS, civil, month_name};
