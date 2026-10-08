//! A minute in UTC as the venue writes one in a description, `YYYYMMDD-HHMM`.
//! There is no clock here: whoever asks whether a minute has passed says when
//! now is, in milliseconds since 1970.

use std::fmt;

use crate::deploy::stamp_minutes;

/// One UTC minute: `20261001-0000` on the wire, `2026-10-01 00:00Z` to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp(i64);

impl Stamp {
    /// Reads `YYYYMMDD-HHMM` exactly, spaces around it aside. `None` for any
    /// other shape and for a day or minute that does not exist.
    #[must_use]
    pub fn parse(text: &str) -> Option<Stamp> {
        stamp_minutes(text.trim(), true).map(Stamp)
    }

    /// Minutes since 1970.
    #[must_use]
    pub const fn from_minutes(minutes: i64) -> Stamp {
        Stamp(minutes)
    }

    #[must_use]
    pub const fn minutes(self) -> i64 {
        self.0
    }

    /// Milliseconds since 1970.
    #[must_use]
    pub const fn ms(self) -> i64 {
        self.0.saturating_mul(60_000)
    }

    /// Whether `now_ms` is at or past this minute.
    #[must_use]
    pub fn passed(self, now_ms: u64) -> bool {
        i128::from(now_ms) >= i128::from(self.ms())
    }

    /// `YYYYMMDD-HHMM`, as a description writes it.
    #[must_use]
    pub fn wire(self) -> String {
        let (year, month, day, hour, minute) = self.parts();
        format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}")
    }

    fn parts(self) -> (i64, i64, i64, i64, i64) {
        let days = self.0.div_euclid(1440);
        let rest = self.0.rem_euclid(1440);
        let (year, month, day) = civil_from_days(days);
        (year, month, day, rest / 60, rest % 60)
    }
}

/// `2026-10-01 00:00Z`.
impl fmt::Display for Stamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day, hour, minute) = self.parts();
        write!(f, "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}Z")
    }
}

/// The day of the proleptic Gregorian calendar `days` after 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
#[path = "tests/time.rs"]
mod tests;
