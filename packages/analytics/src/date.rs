//! Instants and days in UTC, counted from 1970 in integers on the proleptic Gregorian calendar.
//! There is no clock here: whoever asks says when now is. A board's window, a builder's day
//! file, a projection's periods and a payout ledger's lines all name days or instants, and
//! every one of them is worked out here.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Milliseconds in a day.
pub const DAY_MS: i64 = 86_400_000;

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// An instant: milliseconds since 1970 in UTC. Read from RFC 3339 and written as RFC 3339 with
/// milliseconds and a `Z` (`2026-09-01T10:00:00.000Z`), the way hip4-tui's payout ledger holds
/// them, so lines written here and lines written there sort together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct At(i64);

impl At {
    #[must_use]
    pub const fn from_ms(ms: i64) -> At {
        At(ms)
    }

    #[must_use]
    pub const fn ms(self) -> i64 {
        self.0
    }

    /// `YYYY-MM-DDTHH:MM:SS`, a fraction of a second if any (cut to the millisecond), and `Z` or
    /// an offset `+HH:MM`. A space or a lowercase `t` may stand for the `T`, as RFC 3339
    /// allows. `None` for anything else, a day that does not exist included.
    #[must_use]
    pub fn parse(text: &str) -> Option<At> {
        let raw = text.as_bytes();
        let digits = |from: usize, count: usize| -> Option<i64> {
            let field = raw.get(from..from + count)?;
            field.iter().try_fold(0i64, |sum, byte| byte.is_ascii_digit().then(|| sum * 10 + i64::from(byte - b'0')))
        };
        let at = |place: usize, byte: u8| raw.get(place) == Some(&byte);
        let shaped = at(4, b'-') && at(7, b'-') && at(13, b':') && at(16, b':');
        if !shaped || !matches!(raw.get(10), Some(b'T' | b't' | b' ')) {
            return None;
        }
        let (year, month, day) =
            (digits(0, 4)?, u32::try_from(digits(5, 2)?).ok()?, u32::try_from(digits(8, 2)?).ok()?);
        let (hour, minute, second) = (digits(11, 2)?, digits(14, 2)?, digits(17, 2)?);
        if !(1..=12).contains(&month)
            || day == 0
            || day > days_in(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        let mut place = 19;
        let mut millis = 0i64;
        if at(place, b'.') {
            place += 1;
            let start = place;
            while let Some(digit) = raw.get(place).filter(|byte| byte.is_ascii_digit()) {
                if place - start < 3 {
                    millis = millis * 10 + i64::from(digit - b'0');
                }
                place += 1;
            }
            let read = place - start;
            if read == 0 {
                return None;
            }
            for _ in read..3 {
                millis *= 10;
            }
        }
        let offset = match raw.get(place..)? {
            b"Z" | b"z" => 0,
            [sign @ (b'+' | b'-'), _, _, b':', _, _] => {
                let (hours, minutes) = (digits(place + 1, 2)?, digits(place + 4, 2)?);
                if hours > 23 || minutes > 59 {
                    return None;
                }
                let minutes = hours * 60 + minutes;
                if *sign == b'-' { -minutes } else { minutes }
            }
            _ => return None,
        };
        let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset * 60;
        Some(At(seconds * 1_000 + millis))
    }

    /// `days` whole days on, or back when negative.
    #[must_use]
    pub const fn plus_days(self, days: i64) -> At {
        At(self.0.saturating_add(days.saturating_mul(DAY_MS)))
    }

    /// `months` calendar months on, or back, at the same time of day. A day the month does not
    /// have becomes its last: the 31st of January a month on is the 28th of February, or the
    /// 29th in a leap year.
    #[must_use]
    pub fn plus_months(self, months: i64) -> At {
        let (year, month, day) = civil(self);
        let counted = year * 12 + i64::from(month) - 1 + months;
        let (year, month) = (counted.div_euclid(12), counted.rem_euclid(12) + 1);
        let month = u32::try_from(month).unwrap_or(1);
        let day = day.min(days_in(year, month));
        At(days_from_civil(year, month, day) * DAY_MS + self.0.rem_euclid(DAY_MS))
    }

    /// The day, `YYYY-MM-DD`.
    #[must_use]
    pub fn date(self) -> String {
        let (year, month, day) = civil(self);
        format!("{year:04}-{month:02}-{day:02}")
    }

    /// The day, `YYYYMMDD`, as Hyperliquid names a builder's day file.
    #[must_use]
    pub fn compact(self) -> String {
        let (year, month, day) = civil(self);
        format!("{year:04}{month:02}{day:02}")
    }
}

/// The year, month and day of an instant.
#[must_use]
pub fn civil(at: At) -> (i64, u32, u32) {
    civil_from_days(at.0.div_euclid(DAY_MS))
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

/// A month's short English name, `Jan` for 1.
#[must_use]
pub fn month_name(month: u32) -> &'static str {
    month.checked_sub(1).and_then(|at| MONTHS.get(usize::try_from(at).ok()?)).copied().unwrap_or("")
}

impl fmt::Display for At {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil(*self);
        let within = self.0.rem_euclid(DAY_MS);
        let (hour, minute) = (within / 3_600_000, within / 60_000 % 60);
        let (second, millis) = (within / 1_000 % 60, within % 1_000);
        write!(f, "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
    }
}

impl FromStr for At {
    type Err = BadAt;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        At::parse(text).ok_or_else(|| BadAt(text.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an RFC 3339 instant")]
pub struct BadAt(pub String);

impl Serialize for At {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for At {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/date.rs"]
mod tests;
