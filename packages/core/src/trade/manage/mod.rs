//! Orders already resting: modify, cancel, and what stops orders without naming them.

pub mod cancel;
pub mod modify;
pub mod schedule;

pub use cancel::{Cancel, CancelByCloid};
pub use modify::{BatchModify, Change, Modify, OrderRef, modify};
pub use schedule::{Noop, ScheduleCancel, TooSoon};
