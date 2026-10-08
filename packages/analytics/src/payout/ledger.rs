//! The payout ledger: one JSON event a line, appended and never rewritten, and the state they
//! fold to. Its format is the payout tool's, which hip4-tui shares: snake-case types and keys,
//! instants in RFC 3339 with milliseconds, an optional field left out rather than written as
//! `null`. A torn last line is dropped and everything before it stands.
//!
//! A row is claimed before it is sent, so a crash between the two leaves it `submitting`, never
//! unattempted; only the venue's own ledger settles whether a claim paid ([`reconcile`]).

use std::collections::BTreeMap;

use hl_core::{Address, Dec};
use serde::{Deserialize, Serialize};

use super::money;
use crate::{
    date::{At, DAY_MS},
    hip4::LedgerRow,
    math::add,
};

/// Where a row stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    /// Claimed and sent, with the answer not yet known. Only a reconcile resolves it.
    Submitting,
    Sent,
    Failed,
    NeedsReview,
    SkippedDust,
    Dismissed,
}

impl Status {
    pub const ALL: [Status; 7] = [
        Status::Pending,
        Status::Submitting,
        Status::Sent,
        Status::Failed,
        Status::NeedsReview,
        Status::SkippedDust,
        Status::Dismissed,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Submitting => "submitting",
            Status::Sent => "sent",
            Status::Failed => "failed",
            Status::NeedsReview => "needs review",
            Status::SkippedDust => "skipped dust",
            Status::Dismissed => "dismissed",
        }
    }

    /// Still owed: pending, failed, or claimed with no answer yet.
    #[must_use]
    pub const fn is_outstanding(self) -> bool {
        matches!(self, Status::Pending | Status::Failed | Status::Submitting)
    }
}

/// One payout as a batch records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub wallet: Address,
    pub amount_usdc: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub memo: Option<String>,
}

/// One line of the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    BatchCreated {
        at: At,
        batch_id: String,
        source: String,
        treasury: Address,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        account_label: Option<String>,
        rows: Vec<Line>,
    },
    RunStarted {
        at: At,
        run_id: String,
        batch_id: String,
        mode: Mode,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        treasury: Option<Address>,
    },
    RunFinished {
        at: At,
        run_id: String,
        batch_id: String,
        sent: usize,
        failed: usize,
        skipped: usize,
        quarantined: usize,
        sent_usdc: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        stopped_reason: Option<String>,
    },
    RowClaimed {
        at: At,
        batch_id: String,
        wallet: Address,
        run_id: String,
    },
    RowSent {
        at: At,
        batch_id: String,
        wallet: Address,
        run_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        tx_hash: Option<String>,
    },
    RowFailed {
        at: At,
        batch_id: String,
        wallet: Address,
        run_id: String,
        error: String,
    },
    RowStatus {
        at: At,
        batch_id: String,
        wallet: Address,
        status: Status,
        note: String,
    },
    RowHash {
        at: At,
        batch_id: String,
        wallet: Address,
        tx_hash: String,
    },
}

/// Whether a run moved money.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Live,
    Dry,
}

/// One row of a batch, as the events have left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub batch_id: String,
    pub wallet: Address,
    /// Zero where the recorded amount does not read.
    pub amount: Dec,
    /// As recorded, which is what is sent.
    pub amount_usdc: String,
    pub line: usize,
    pub memo: Option<String>,
    pub status: Status,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub tx_hash: Option<String>,
    pub created_at: At,
    pub submitted_at: Option<At>,
    pub sent_at: Option<At>,
    /// The run that sent it, or `reconcile`.
    pub sent_by: Option<String>,
}

/// One batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    pub batch_id: String,
    /// The file it came from.
    pub source: String,
    pub created_at: At,
    pub treasury: Address,
    pub account_label: Option<String>,
    pub row_count: usize,
    pub total: Dec,
}

/// One finished run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub run_id: String,
    pub batch_id: String,
    /// When it finished.
    pub at: At,
    pub started_at: Option<At>,
    pub sent: usize,
    pub failed: usize,
    pub skipped: usize,
    pub quarantined: usize,
    pub sent_usdc: String,
    pub stopped_reason: Option<String>,
}

/// What a ledger folds to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// In the order they were created.
    pub batches: Vec<Batch>,
    rows: BTreeMap<String, Vec<Row>>,
    pub runs: Vec<Run>,
}

/// One paying account's figures, across its batches.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Totals {
    pub batches: usize,
    pub rows_sent: usize,
    pub sent: Dec,
    pub outstanding: usize,
    pub needs_review: usize,
    pub first_batch_at: Option<At>,
    pub last_payout_at: Option<At>,
}

impl State {
    #[must_use]
    pub fn batch(&self, batch_id: &str) -> Option<&Batch> {
        self.batches.iter().find(|batch| batch.batch_id == batch_id)
    }

    /// The batches, the newest first.
    #[must_use]
    pub fn newest(&self) -> Vec<&Batch> {
        let mut listed: Vec<&Batch> = self.batches.iter().collect();
        listed.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        listed
    }

    #[must_use]
    pub fn rows(&self, batch_id: &str) -> &[Row] {
        self.rows.get(batch_id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn has(&self, batch_id: &str) -> bool {
        self.rows.contains_key(batch_id)
    }

    /// The rows a run may send: pending, and failed with attempts left.
    #[must_use]
    pub fn payable(&self, batch_id: &str, max_attempts: u32) -> Vec<&Row> {
        self.rows(batch_id).iter().filter(|row| owed(row, max_attempts)).collect()
    }

    #[must_use]
    pub fn runs_on(&self, batch_id: &str) -> Vec<&Run> {
        self.runs.iter().filter(|run| run.batch_id == batch_id).collect()
    }

    /// Which batch id a file resumes under. A legacy id, hashed from the rows alone, is adopted
    /// only when the account paying now is the one that paid then.
    #[must_use]
    pub fn resolve(&self, scoped: &str, legacy: &str, treasury: Address) -> String {
        if self.has(scoped) {
            return scoped.to_owned();
        }
        match self.batch(legacy) {
            Some(batch) if batch.treasury == treasury => legacy.to_owned(),
            _ => scoped.to_owned(),
        }
    }

    /// Where this wallet was already paid this amount in the `days` before `now`, in another
    /// batch, the earliest first. Not scoped to one account: a repeat across budgets is the
    /// point.
    #[must_use]
    pub fn recently_paid(&self, wallet: Address, amount: Dec, days: i64, except: &str, now: At) -> Vec<&Row> {
        let cutoff = At::from_ms(now.ms().saturating_sub(days.saturating_mul(DAY_MS)));
        let mut hits: Vec<&Row> = self
            .rows
            .iter()
            .filter(|(batch_id, _)| batch_id.as_str() != except)
            .flat_map(|(_, rows)| rows)
            .filter(|row| {
                row.status == Status::Sent
                    && row.wallet == wallet
                    && row.amount == amount
                    && row.sent_at.unwrap_or(row.created_at) >= cutoff
            })
            .collect();
        hits.sort_by_key(|row| row.sent_at.unwrap_or(row.created_at));
        hits
    }

    /// Each paying account's figures.
    #[must_use]
    pub fn totals(&self) -> BTreeMap<Address, Totals> {
        let mut totals: BTreeMap<Address, Totals> = BTreeMap::new();
        for batch in &self.batches {
            let entry = totals.entry(batch.treasury).or_default();
            entry.batches += 1;
            entry.first_batch_at = Some(entry.first_batch_at.map_or(batch.created_at, |had| had.min(batch.created_at)));
            for row in self.rows(&batch.batch_id) {
                match row.status {
                    Status::Sent => {
                        entry.rows_sent += 1;
                        entry.sent = add(entry.sent, row.amount);
                        if let Some(at) = row.sent_at {
                            entry.last_payout_at = Some(entry.last_payout_at.map_or(at, |had| had.max(at)));
                        }
                    }
                    Status::NeedsReview => entry.needs_review += 1,
                    status if status.is_outstanding() => entry.outstanding += 1,
                    _ => {}
                }
            }
        }
        totals
    }

    fn row_mut(&mut self, batch_id: &str, wallet: Address) -> Option<&mut Row> {
        self.rows.get_mut(batch_id)?.iter_mut().find(|row| row.wallet == wallet)
    }
}

/// Owed and selectable: pending, or failed with attempts left.
pub(crate) fn owed(row: &Row, max_attempts: u32) -> bool {
    match row.status {
        Status::Pending => true,
        Status::Failed => row.attempts < max_attempts,
        _ => false,
    }
}

/// How many rows stand at each status, every status named.
#[must_use]
pub fn tally(rows: &[Row]) -> BTreeMap<&'static str, usize> {
    let mut counts: BTreeMap<&'static str, usize> = Status::ALL.into_iter().map(|one| (one.label(), 0)).collect();
    for row in rows {
        *counts.entry(row.status.label()).or_default() += 1;
    }
    counts
}

/// What is still owed on these rows.
#[must_use]
pub fn outstanding(rows: &[Row]) -> Dec {
    money::total(rows.iter().filter(|row| row.status.is_outstanding()).map(|row| row.amount))
}

/// A ledger's text, one event a line. A line that does not read, a torn last one included, is
/// dropped; everything else stands.
#[must_use]
pub fn read(text: &str) -> Vec<Event> {
    text.lines().filter(|line| !line.trim().is_empty()).filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// One event as the line appended for it, its line break included.
pub fn line(event: &Event) -> Result<String, serde_json::Error> {
    serde_json::to_string(event).map(|line| format!("{line}\n"))
}

/// The events in order. A batch created twice keeps its first; an event for a batch never
/// created is ignored.
#[must_use]
pub fn fold(events: &[Event]) -> State {
    let mut state = State::default();
    let mut started: BTreeMap<&str, At> = BTreeMap::new();
    for event in events {
        match event {
            Event::BatchCreated { at, batch_id, source, treasury, account_label, rows } => {
                if state.has(batch_id) {
                    continue;
                }
                let listed: Vec<Row> = rows
                    .iter()
                    .map(|line| Row {
                        batch_id: batch_id.clone(),
                        wallet: line.wallet,
                        amount: money::parse(&line.amount_usdc).unwrap_or(Dec::ZERO),
                        amount_usdc: line.amount_usdc.clone(),
                        line: line.line,
                        memo: line.memo.clone(),
                        status: Status::Pending,
                        attempts: 0,
                        last_error: None,
                        tx_hash: None,
                        created_at: *at,
                        submitted_at: None,
                        sent_at: None,
                        sent_by: None,
                    })
                    .collect();
                state.batches.push(Batch {
                    batch_id: batch_id.clone(),
                    source: source.clone(),
                    created_at: *at,
                    treasury: *treasury,
                    account_label: account_label.clone(),
                    row_count: listed.len(),
                    total: money::total(listed.iter().map(|row| row.amount)),
                });
                state.rows.insert(batch_id.clone(), listed);
            }
            Event::RunStarted { at, run_id, .. } => {
                started.insert(run_id, *at);
            }
            Event::RunFinished {
                at,
                run_id,
                batch_id,
                sent,
                failed,
                skipped,
                quarantined,
                sent_usdc,
                stopped_reason,
            } => state.runs.push(Run {
                run_id: run_id.clone(),
                batch_id: batch_id.clone(),
                at: *at,
                started_at: started.get(run_id.as_str()).copied(),
                sent: *sent,
                failed: *failed,
                skipped: *skipped,
                quarantined: *quarantined,
                sent_usdc: sent_usdc.clone(),
                stopped_reason: stopped_reason.clone(),
            }),
            Event::RowClaimed { at, batch_id, wallet, run_id } => {
                if let Some(row) = state.row_mut(batch_id, *wallet) {
                    row.status = Status::Submitting;
                    row.attempts = row.attempts.saturating_add(1);
                    row.submitted_at = Some(*at);
                    row.sent_by = Some(run_id.clone());
                }
            }
            Event::RowSent { at, batch_id, wallet, run_id, tx_hash } => {
                if let Some(row) = state.row_mut(batch_id, *wallet) {
                    row.status = Status::Sent;
                    row.sent_at = Some(*at);
                    row.last_error = None;
                    row.sent_by = Some(run_id.clone());
                    if let Some(hash) = tx_hash {
                        row.tx_hash = Some(hash.clone());
                    }
                }
            }
            Event::RowFailed { batch_id, wallet, error, .. } => {
                if let Some(row) = state.row_mut(batch_id, *wallet) {
                    row.status = Status::Failed;
                    row.last_error = Some(error.clone());
                }
            }
            Event::RowStatus { batch_id, wallet, status, note, .. } => {
                if let Some(row) = state.row_mut(batch_id, *wallet) {
                    row.status = *status;
                    row.last_error = Some(note.clone());
                }
            }
            Event::RowHash { batch_id, wallet, tx_hash, .. } => {
                if let Some(row) = state.row_mut(batch_id, *wallet) {
                    row.tx_hash = Some(tx_hash.clone());
                }
            }
        }
    }
    state
}

/// USDC that left the treasury, as its ledger records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub hash: String,
    /// Milliseconds.
    pub time: u64,
    pub destination: Address,
    pub amount: Dec,
}

/// The USDC `spotTransfer` and `send` rows that left `from`: what a reconcile looks for.
#[must_use]
pub fn transfers(rows: &[LedgerRow], from: Address) -> Vec<Transfer> {
    rows.iter()
        .filter_map(|row| {
            let delta = &row.delta;
            if !matches!(delta.kind.as_str(), "spotTransfer" | "send") || !delta.token.as_deref()?.starts_with("USDC") {
                return None;
            }
            if delta.user.as_deref()?.parse::<Address>().ok()? != from {
                return None;
            }
            Some(Transfer {
                hash: row.hash.clone(),
                time: row.time,
                destination: delta.destination.as_deref()?.parse().ok()?,
                amount: Dec::parse(delta.amount.as_deref()?).ok()?,
            })
        })
        .collect()
}

/// The first claim still waiting on an answer: where to read the treasury's ledger from. `None`
/// when nothing waits.
#[must_use]
pub fn waiting_since(state: &State, batch_id: &str) -> Option<At> {
    state.rows(batch_id).iter().filter(|row| row.status == Status::Submitting).filter_map(|row| row.submitted_at).min()
}

/// What the venue's ledger says of each claim still waiting: sent, with the transfer's hash,
/// where a transfer of its amount to its wallet is there, each transfer answering for one claim;
/// set aside for a person otherwise. The events to append, at `now`.
#[must_use]
pub fn reconcile(state: &State, batch_id: &str, transfers: &[Transfer], now: At) -> Vec<Event> {
    let mut taken: Vec<&str> = Vec::new();
    let mut events = Vec::new();
    for row in state.rows(batch_id).iter().filter(|row| row.status == Status::Submitting) {
        let found = transfers.iter().find(|transfer| {
            !taken.contains(&transfer.hash.as_str())
                && transfer.destination == row.wallet
                && transfer.amount == row.amount
        });
        events.push(match found {
            Some(transfer) => {
                taken.push(&transfer.hash);
                Event::RowSent {
                    at: now,
                    batch_id: batch_id.to_owned(),
                    wallet: row.wallet,
                    run_id: "reconcile".to_owned(),
                    tx_hash: Some(transfer.hash.clone()),
                }
            }
            None => Event::RowStatus {
                at: now,
                batch_id: batch_id.to_owned(),
                wallet: row.wallet,
                status: Status::NeedsReview,
                note: format!("no transfer of {} to this address is in the venue's ledger", money::wire(row.amount)),
            },
        });
    }
    events
}

#[cfg(test)]
#[path = "../tests/ledger.rs"]
mod tests;
