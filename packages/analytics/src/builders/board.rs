//! The builder board: every builder Hydromancer ranks, by outcome volume over a window, folded
//! from the one packed matrix `outcome-builder-volume-daily` answers (a builder a row, a day a
//! column). A window is the newest days of that matrix, so 24h, 7d, 30d and all are suffixes of
//! one read. The fills no builder routed are a row of their own, ranked by volume beside the
//! builders and holding no place: trading direct is not a builder.

use hl_core::{Address, Dec};
use hl_hydromancer::VolumeDaily;

use crate::{
    date::At,
    math::{add, share, sum},
};

/// A board's window, the newest days of the matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Window {
    /// The newest column: today so far in UTC, not a rolling 24 hours.
    Day,
    Week,
    Month,
    All,
}

impl Window {
    pub const ALL: [Window; 4] = [Window::Day, Window::Week, Window::Month, Window::All];

    #[must_use]
    pub const fn days(self) -> usize {
        match self {
            Window::Day => 1,
            Window::Week => 7,
            Window::Month => 30,
            Window::All => 364,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Window::Day => "24h",
            Window::Week => "7d",
            Window::Month => "30d",
            Window::All => "all",
        }
    }

    /// The `startDate` (`YYYY-MM-DD`) that reaches back a whole [`Window::All`] from `now`'s day.
    #[must_use]
    pub fn since(now: At) -> String {
        let back = i64::try_from(Window::All.days()).unwrap_or(364) - 1;
        now.plus_days(-back).date()
    }

    const fn index(self) -> usize {
        match self {
            Window::Day => 0,
            Window::Week => 1,
            Window::Month => 2,
            Window::All => 3,
        }
    }
}

/// Hundredths of a percent in the whole: a share of 3125 is 31.25%.
pub const SHARE_UNITS: u32 = 10_000;

/// One row of a window: a builder, or the fills no builder routed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// `None` for the fills no builder routed.
    pub builder: Option<Address>,
    /// Notional, in the quote.
    pub volume: Dec,
    pub fills: u64,
    /// The busiest day's traders: a peak, not a sum, since one trader trades on many days.
    pub peak_users: u64,
    /// What the window's fills paid the builder, in the quote.
    pub fees: Dec,
    /// Its place among the builders; `None` for the fills no builder routed.
    pub rank: Option<u32>,
    /// Of the window's whole volume, in hundredths of a percent ([`SHARE_UNITS`]), cut down.
    pub share: u32,
    /// Volume a day over the window, oldest first.
    pub daily: Vec<Dec>,
}

impl Standing {
    /// The fills no builder routed.
    #[must_use]
    pub const fn is_direct(&self) -> bool {
        self.builder.is_none()
    }
}

/// Why a matrix is not folded: a short row would under-report the builder it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the daily answer is ragged: a short row would under-report the builder it belongs to")]
pub struct Ragged;

/// Every window of one matrix, ranked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    tables: [Vec<Standing>; 4],
    totals: [Dec; 4],
    fees: [Dec; 4],
    /// The matrix's days, `YYYY-MM-DD`, oldest first.
    pub dates: Vec<String>,
}

impl Board {
    /// Each window's rows, busiest first, the builders ranked among themselves. A row with no
    /// volume and no fills in a window is not in it.
    pub fn of(daily: &VolumeDaily) -> Result<Board, Ragged> {
        if !daily.is_aligned() {
            return Err(Ragged);
        }
        let mut board = Board {
            tables: std::array::from_fn(|_| Vec::new()),
            totals: [Dec::ZERO; 4],
            fees: [Dec::ZERO; 4],
            dates: daily.dates.clone(),
        };
        let days = daily.dates.len();
        let series = &daily.series;
        for window in Window::ALL {
            let from = days.saturating_sub(window.days());
            let at = window.index();
            for (row, builder) in daily.builders.iter().enumerate() {
                let (Some(volume), Some(fees), Some(fills), Some(users)) = (
                    series.volume.get(row).and_then(|one| one.get(from..)),
                    series.builder_fee.get(row).and_then(|one| one.get(from..)),
                    series.fills.get(row).and_then(|one| one.get(from..)),
                    series.unique_users.get(row).and_then(|one| one.get(from..)),
                ) else {
                    return Err(Ragged);
                };
                let standing = Standing {
                    builder: *builder,
                    volume: sum(volume),
                    fills: fills.iter().fold(0u64, |sum, one| sum.saturating_add(*one)),
                    peak_users: users.iter().copied().max().unwrap_or(0),
                    fees: sum(fees),
                    rank: None,
                    share: 0,
                    daily: volume.to_vec(),
                };
                if standing.volume.is_zero() && standing.fills == 0 {
                    continue;
                }
                board.totals[at] = add(board.totals[at], standing.volume);
                board.fees[at] = add(board.fees[at], standing.fees);
                board.tables[at].push(standing);
            }
            rank(&mut board.tables[at], board.totals[at]);
        }
        Ok(board)
    }

    #[must_use]
    pub fn table(&self, window: Window) -> &[Standing] {
        &self.tables[window.index()]
    }

    /// The window's whole volume, every row's.
    #[must_use]
    pub fn total(&self, window: Window) -> Dec {
        self.totals[window.index()]
    }

    /// What the window's fills paid builders, every row's.
    #[must_use]
    pub fn fees(&self, window: Window) -> Dec {
        self.fees[window.index()]
    }

    /// The builders ranked `count` or better, and the fills no builder routed where they fall
    /// among them.
    #[must_use]
    pub fn top(&self, window: Window, count: u32) -> Vec<&Standing> {
        self.table(window).iter().filter(|row| row.rank.is_none_or(|rank| rank <= count)).collect()
    }

    /// How many builders the window ranks.
    #[must_use]
    pub fn builders(&self, window: Window) -> usize {
        self.table(window).iter().filter(|row| !row.is_direct()).count()
    }

    #[must_use]
    pub fn find(&self, window: Window, builder: Address) -> Option<&Standing> {
        self.table(window).iter().find(|row| row.builder == Some(builder))
    }
}

/// Busiest first, an address breaking a tie; places for the builders alone, and each row's share
/// of the whole.
fn rank(rows: &mut [Standing], total: Dec) {
    rows.sort_by(|a, b| b.volume.cmp(&a.volume).then_with(|| a.builder.cmp(&b.builder)));
    let mut place = 0u32;
    for row in rows.iter_mut() {
        if !row.is_direct() {
            place = place.saturating_add(1);
            row.rank = Some(place);
        }
        row.share = share(row.volume, total);
    }
}

#[cfg(test)]
#[path = "../tests/board.rs"]
mod tests;
