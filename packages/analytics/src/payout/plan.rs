//! A plan: what a batch still owes, what the guards divert, what may be sent and in which slices,
//! and every reason the run may not start. Made before anything moves, and the treasury checked
//! again before each slice ([`covers`]), so a run that cannot pay the next slice stops there with
//! no attempts spent.

use hl_core::Dec;

use super::{
    config::{Config, Guard},
    ledger::{Row, State, owed},
    money,
};
use crate::date::At;

/// What the treasury holds, read before a run and again before each slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Treasury {
    pub spot_usdc: Dec,
    pub spot_hype: Dec,
    pub perp_withdrawable: Dec,
}

/// A row whose wallet was paid the same amount lately, in another batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeat {
    /// The row, by its place in the batch.
    pub at: usize,
    /// How many earlier payments match.
    pub earlier: usize,
    pub earlier_batch: String,
    /// The account that paid it, by its label or address.
    pub earlier_account: String,
    pub when: At,
}

/// Why an owed row is not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diverted {
    /// Below the floor.
    Dust,
    /// Above the ceiling, for a person to review.
    Oversized,
}

/// A batch planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub batch_id: String,
    /// Rows owed, by their place in the batch.
    pub payable: Vec<usize>,
    pub diverted: Vec<(usize, Diverted)>,
    pub sendable: Vec<usize>,
    /// What the sendable rows come to.
    pub outstanding: Dec,
    pub repeats: Vec<Repeat>,
    /// Every reason the run may not start.
    pub blockers: Vec<String>,
    /// The sendable rows cut into runs of value, each its own balance check.
    pub slices: Vec<Vec<usize>>,
}

impl Plan {
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        !self.blockers.is_empty()
    }

    /// Whether a run may start: nothing blocks it and something is to be sent.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.is_blocked() && !self.sendable.is_empty()
    }

    #[must_use]
    pub fn diverted_by(&self, why: Diverted) -> Vec<usize> {
        self.diverted.iter().filter(|(_, had)| *had == why).map(|(at, _)| *at).collect()
    }

    /// Repeats an account other than `paying` paid: the mix-up separate budgets exist to catch.
    #[must_use]
    pub fn repeats_elsewhere(&self, paying: &str) -> usize {
        self.repeats.iter().filter(|repeat| repeat.earlier_account != paying).count()
    }
}

/// A total the person declares the file comes to, typed, to be checked against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expected<'a>(pub &'a str);

/// Plans `batch_id` against the guards, the file's own total, a declared total if any, and the
/// treasury where it was read. An unread treasury blocks nothing: the slice check reads it again.
#[must_use]
pub fn plan(
    state: &State,
    batch_id: &str,
    config: &Config,
    sheet_total: Dec,
    expect: Option<Expected<'_>>,
    treasury: Option<Treasury>,
    now: At,
) -> Plan {
    let rows = state.rows(batch_id);
    let payable: Vec<usize> =
        rows.iter().enumerate().filter(|(_, row)| owed(row, config.max_attempts)).map(|(at, _)| at).collect();
    let amount = |at: &usize| rows.get(*at).map_or(Dec::ZERO, |row| row.amount);

    let floor = config.limit(Guard::MinPayout);
    let ceiling = config.limit(Guard::MaxSinglePayout);
    let mut diverted = Vec::new();
    let mut sendable = Vec::new();
    for at in &payable {
        let due = amount(at);
        if floor.is_some_and(|floor| due < floor) {
            diverted.push((*at, Diverted::Dust));
        } else if ceiling.is_some_and(|ceiling| due > ceiling) {
            diverted.push((*at, Diverted::Oversized));
        } else {
            sendable.push(*at);
        }
    }
    let outstanding = money::total(sendable.iter().map(amount));
    let repeats = match config.duplicate_window_days > 0 {
        true => look_back(state, batch_id, rows, &sendable, config.duplicate_window_days, now),
        false => Vec::new(),
    };

    let mut blockers = config.problems();
    if let Some(Expected(said)) = expect {
        match money::parse(said) {
            Err(_) => blockers.push(format!("`{said}` is not a decimal amount")),
            Ok(wanted) if wanted != sheet_total => {
                blockers.push(format!("the declared total {said} is not the file's total {}", money::wire(sheet_total)))
            }
            Ok(_) => {}
        }
    }
    if let Some(cap) = config.limit(Guard::MaxCsvTotal)
        && sheet_total > cap
    {
        blockers.push(format!("the file totals {}, over the {} ceiling", money::wire(sheet_total), money::wire(cap)));
    }
    if let (Some(treasury), Some(least)) = (treasury, config.limit(Guard::MinHypeBalance))
        && treasury.spot_hype < least
    {
        blockers.push(format!(
            "the treasury holds {} HYPE, under the {} required",
            money::wire(treasury.spot_hype),
            money::wire(least)
        ));
    }

    let slices = slice(&sendable, rows, config.limit(Guard::MaxBatchTotal));
    Plan { batch_id: batch_id.to_owned(), payable, diverted, sendable, outstanding, repeats, blockers, slices }
}

/// Each sendable row whose wallet was paid the same amount in the window, in another batch.
fn look_back(state: &State, batch_id: &str, rows: &[Row], sendable: &[usize], days: i64, now: At) -> Vec<Repeat> {
    sendable
        .iter()
        .filter_map(|at| {
            let row = rows.get(*at)?;
            let earlier = state.recently_paid(row.wallet, row.amount, days, batch_id, now);
            let first = earlier.first()?;
            let named = state.batch(&first.batch_id).map_or_else(
                || "an account this ledger does not name".to_owned(),
                |batch| batch.account_label.clone().unwrap_or_else(|| batch.treasury.to_hex()),
            );
            Some(Repeat {
                at: *at,
                earlier: earlier.len(),
                earlier_batch: first.batch_id.clone(),
                earlier_account: named,
                when: first.sent_at.unwrap_or(first.created_at),
            })
        })
        .collect()
}

/// Cuts the rows into runs of value, each no more than `cap`: a slice is the unit the treasury's
/// balance is checked against, and a row over the cap is a slice of its own.
fn slice(sendable: &[usize], rows: &[Row], cap: Option<Dec>) -> Vec<Vec<usize>> {
    let Some(cap) = cap else {
        return match sendable.is_empty() {
            true => Vec::new(),
            false => vec![sendable.to_vec()],
        };
    };
    let mut slices: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut running = Dec::ZERO;
    for at in sendable {
        let amount = rows.get(*at).map_or(Dec::ZERO, |row| row.amount);
        let next = running.checked_add(amount).unwrap_or(running);
        if !current.is_empty() && next > cap {
            slices.push(std::mem::take(&mut current));
            running = amount;
        } else {
            running = next;
        }
        current.push(*at);
    }
    if !current.is_empty() {
        slices.push(current);
    }
    slices
}

/// Every reason the treasury cannot pay a slice of `slice_total` now.
#[must_use]
pub fn covers(treasury: Treasury, slice_total: Dec, config: &Config) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(least) = config.limit(Guard::MinHypeBalance)
        && treasury.spot_hype < least
    {
        problems.push(format!("HYPE {} is under the {} required", money::wire(treasury.spot_hype), money::wire(least)));
    }
    if treasury.spot_usdc < slice_total {
        problems.push(format!(
            "spot USDC {} does not cover {}",
            money::wire(treasury.spot_usdc),
            money::wire(slice_total)
        ));
    }
    problems
}

#[cfg(test)]
#[path = "../tests/plan.rs"]
mod tests;
