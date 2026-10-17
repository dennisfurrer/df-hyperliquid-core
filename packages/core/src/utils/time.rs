//! A UTC minute as the venue writes one in an outcome's description, `YYYYMMDD-HHMM`, counted
//! from 1970 in integers on the proleptic Gregorian calendar. There is no clock here: the caller
//! passes now, in milliseconds.

use std::fmt;

const MINUTE_MS: i64 = 60_000;

/// One UTC minute: `20261001-0000` on the wire, `2026-10-01 00:00Z` to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp(i64);

impl Stamp {
    /// Reads `YYYYMMDD-HHMM` exactly, spaces around it aside. `None` for any other shape and for
    /// a day or minute that does not exist.
    #[must_use]
    pub fn parse(text: &str) -> Option<Stamp> {
        Stamp::exact(text.trim())
    }

    /// `YYYYMMDD-HHMM` and nothing else, not even a space: `20260231-1200`, `20261001-2400` and
    /// `20261001-1260` are no minute.
    #[must_use]
    pub fn exact(text: &str) -> Option<Stamp> {
        let raw = text.as_bytes();
        if raw.len() != 13 || raw.get(8) != Some(&b'-') {
            return None;
        }
        let (year, month, day) = (digits(raw, 0, 4)?, digits(raw, 4, 2)?, digits(raw, 6, 2)?);
        minutes_since_1970(year, month, day, digits(raw, 9, 2)?, digits(raw, 11, 2)?).map(Stamp)
    }

    /// A day as `YYYYMMDD` and nothing else, at its midnight.
    #[must_use]
    pub fn day(text: &str) -> Option<Stamp> {
        let raw = text.as_bytes();
        if raw.len() != 8 {
            return None;
        }
        minutes_since_1970(digits(raw, 0, 4)?, digits(raw, 4, 2)?, digits(raw, 6, 2)?, 0, 0).map(Stamp)
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
        self.0.saturating_mul(MINUTE_MS)
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

    fn parts(self) -> (i64, u32, u32, i64, i64) {
        let (year, month, day) = civil_from_days(self.0.div_euclid(1_440));
        let within = self.0.rem_euclid(1_440);
        (year, month, day, within / 60, within % 60)
    }
}

/// `2026-10-01 00:00Z`.
impl fmt::Display for Stamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day, hour, minute) = self.parts();
        write!(f, "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}Z")
    }
}

/// `count` ASCII digits from `from`, as a number; `None` where any is not a digit.
fn digits(raw: &[u8], from: usize, count: usize) -> Option<i64> {
    let field = raw.get(from..from.checked_add(count)?)?;
    field.iter().try_fold(0i64, |sum, byte| byte.is_ascii_digit().then(|| sum * 10 + i64::from(byte - b'0')))
}

/// Minutes from 1970 to a minute of the calendar; `None` for one that does not exist.
fn minutes_since_1970(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> Option<i64> {
    let month = u32::try_from(month).ok().filter(|month| (1..=12).contains(month))?;
    let day = u32::try_from(day).ok().filter(|day| (1..=days_in(year, month)).contains(day))?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return None;
    }
    Some((days_from_civil(year, month, day) * 24 + hour) * 60 + minute)
}

/// The day of the calendar `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400;
    let year = if month <= 2 { year + 1 } else { year };
    (year, u32::try_from(month).unwrap_or(1), u32::try_from(day).unwrap_or(1))
}

/// Days from 1970-01-01 to a day of the calendar (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let (month, day) = (i64::from(month), i64::from(day));
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

const fn days_in(year: i64, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
#[path = "tests/time.rs"]
mod tests;
