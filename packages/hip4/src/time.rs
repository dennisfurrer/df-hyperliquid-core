//! A minute in UTC as the venue writes one in a description, `YYYYMMDD-HHMM`:
//! hl-core's calendar. There is no clock here: whoever asks whether a minute
//! has passed says when now is, in milliseconds since 1970.

pub use hl_core::time::Stamp;
