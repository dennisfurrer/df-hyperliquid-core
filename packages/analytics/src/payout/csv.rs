//! A payout file as text: a header naming a wallet column and an amount column (and a memo, if
//! any) in any of the spellings people use, then a row a payout. Read whole or refused whole: a
//! row that will not read, or a wallet named twice unless merging was asked for, refuses the
//! file, because guessing pays the wrong amount. Written back in one spelling ([`write()`]).

use std::collections::BTreeMap;

use hl_core::{Address, Dec};
use sha2::{Digest, Sha256};

use super::{ledger::Line, money};

pub const WALLET_HEADERS: [&str; 6] = ["wallet", "address", "recipient", "destination", "to", "wallet_address"];

pub const AMOUNT_HEADERS: [&str; 8] =
    ["amount_usdc", "amount", "usdc", "reward_usdc", "reward", "payout", "payout_usdc", "value"];

pub const MEMO_HEADERS: [&str; 7] = ["memo", "note", "notes", "reference", "label", "market", "market_name"];

/// Hex digits in a batch id.
const ID_LENGTH: usize = 16;

/// One payout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub wallet: Address,
    /// USDC, six places at most.
    pub amount: Dec,
    /// Its line in the file; the header is line one.
    pub line: usize,
    pub memo: Option<String>,
}

impl Row {
    /// The amount as the payout writes it.
    #[must_use]
    pub fn said(&self) -> String {
        money::wire(self.amount)
    }
}

/// A wallet named on more than one line, summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    pub wallet: Address,
    pub lines: Vec<usize>,
    pub total: Dec,
}

/// Which columns were read, as the header spelt them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Columns {
    pub wallet: String,
    pub amount: String,
    pub memo: Option<String>,
}

/// A file read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub rows: Vec<Row>,
    pub merged: Vec<Merged>,
    /// Rows of nothing, by wallet and line: not paid, and not an error.
    pub zeroes: Vec<(Address, usize)>,
    pub total: Dec,
    /// The payout set's own identity: the same wallets and amounts whatever the file is called or
    /// what order its lines are in.
    pub fingerprint: String,
    /// The id the payout tool gave a batch before ids named the paying account.
    pub legacy_id: String,
    pub columns: Columns,
}

impl Sheet {
    /// The batch this file is for `treasury`: re-running it resumes the batch rather than paying
    /// it again.
    #[must_use]
    pub fn batch_id(&self, treasury: Address) -> String {
        batch_id(&self.fingerprint, treasury)
    }

    /// The rows as a batch records them.
    #[must_use]
    pub fn lines(&self) -> Vec<Line> {
        self.rows
            .iter()
            .map(|row| Line { wallet: row.wallet, amount_usdc: row.said(), line: row.line, memo: row.memo.clone() })
            .collect()
    }
}

/// The first sixteen hex digits of SHA-256 over the paying account and the fingerprint.
#[must_use]
pub fn batch_id(fingerprint: &str, treasury: Address) -> String {
    digest(&format!("{}\n{fingerprint}", treasury.to_hex()))
}

fn digest(text: &str) -> String {
    let hashed = Sha256::digest(text.as_bytes());
    let mut hex = String::with_capacity(ID_LENGTH);
    for byte in hashed.iter().take(ID_LENGTH / 2) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadSheet {
    #[error("the file is empty")]
    Empty,
    #[error("no payable rows")]
    Nothing,
    #[error(
        "could not find the columns; the header was [{header}]\n  wallet: one of {wallets}\n  amount: one of {amounts}"
    )]
    NoColumns { header: String, wallets: String, amounts: String },
    #[error("{} unusable row{}", .0.len(), if .0.len() == 1 { "" } else { "s" })]
    Unusable(Vec<String>),
    #[error("{} wallet{} named more than once", .0.len(), if .0.len() == 1 { "" } else { "s" })]
    Duplicated(Vec<String>),
}

impl BadSheet {
    /// Each row's trouble, where there is a list of them.
    #[must_use]
    pub fn detail(&self) -> &[String] {
        match self {
            BadSheet::Unusable(lines) | BadSheet::Duplicated(lines) => lines,
            _ => &[],
        }
    }
}

/// Reads a payout file. `merge` sums the lines that name one wallet twice; without it a
/// repeated wallet refuses the file.
pub fn read(text: &str, merge: bool) -> Result<Sheet, BadSheet> {
    let table: Vec<Vec<String>> =
        split(text).into_iter().filter(|row| row.iter().any(|cell| !cell.trim().is_empty())).collect();
    let header = table.first().ok_or(BadSheet::Empty)?;
    let named: Vec<String> = header.iter().map(|cell| normalise(cell)).collect();
    let at = |names: &[&str]| named.iter().position(|cell| names.contains(&cell.as_str()));
    let (Some(wallet_at), Some(amount_at)) = (at(&WALLET_HEADERS), at(&AMOUNT_HEADERS)) else {
        return Err(BadSheet::NoColumns {
            header: named.join(", "),
            wallets: WALLET_HEADERS.join(", "),
            amounts: AMOUNT_HEADERS.join(", "),
        });
    };
    let memo_at = at(&MEMO_HEADERS);

    let mut problems = Vec::new();
    let mut rows = Vec::new();
    let mut zeroes = Vec::new();
    for (index, cells) in table.iter().enumerate().skip(1) {
        let line = index + 1;
        let cell = |at: usize| cells.get(at).map(|cell| cell.trim()).unwrap_or_default();
        let (said_wallet, said_amount) = (cell(wallet_at), cell(amount_at));
        if said_wallet.is_empty() && said_amount.is_empty() {
            continue;
        }
        let Ok(wallet) = said_wallet.parse::<Address>() else {
            problems.push(format!("line {line}: `{said_wallet}` is not an address"));
            continue;
        };
        let Ok(amount) = money::parse(said_amount) else {
            problems.push(format!("line {line}: `{said_amount}` is not an amount of USDC"));
            continue;
        };
        if amount.is_zero() {
            zeroes.push((wallet, line));
            continue;
        }
        let memo = memo_at.map(cell).filter(|memo| !memo.is_empty()).map(str::to_owned);
        rows.push(Row { wallet, amount, line, memo });
    }
    if !problems.is_empty() {
        return Err(BadSheet::Unusable(problems));
    }
    if rows.is_empty() {
        return Err(BadSheet::Nothing);
    }

    let mut by_wallet: BTreeMap<Address, Vec<&Row>> = BTreeMap::new();
    for row in &rows {
        by_wallet.entry(row.wallet).or_default().push(row);
    }
    let mut merged = Vec::new();
    if by_wallet.values().any(|listed| listed.len() > 1) {
        if !merge {
            let mut said: Vec<String> = by_wallet
                .iter()
                .filter(|(_, listed)| listed.len() > 1)
                .map(|(wallet, listed)| {
                    let lines: Vec<String> = listed.iter().map(|row| row.line.to_string()).collect();
                    let amounts: Vec<String> = listed.iter().map(|row| row.said()).collect();
                    format!("{} on lines {} ({})", wallet.to_hex(), lines.join(", "), amounts.join(" + "))
                })
                .collect();
            said.push("merge them into one send, or fix the file".to_owned());
            return Err(BadSheet::Duplicated(said));
        }
        let mut summed: Vec<Row> = Vec::with_capacity(by_wallet.len());
        for (wallet, listed) in &by_wallet {
            let total = money::total(listed.iter().map(|row| row.amount));
            let Some(first) = listed.first() else { continue };
            summed.push(Row { wallet: *wallet, amount: total, line: first.line, memo: first.memo.clone() });
            if listed.len() > 1 {
                merged.push(Merged { wallet: *wallet, lines: listed.iter().map(|row| row.line).collect(), total });
            }
        }
        summed.sort_by_key(|row| row.line);
        merged.sort_by_key(|one| one.lines.first().copied().unwrap_or(0));
        rows = summed;
    }

    let total = money::total(rows.iter().map(|row| row.amount));
    let fingerprint = fingerprint(&rows);
    Ok(Sheet {
        merged,
        zeroes,
        total,
        legacy_id: digest(&fingerprint),
        fingerprint,
        columns: Columns {
            wallet: header.get(wallet_at).cloned().unwrap_or_default(),
            amount: header.get(amount_at).cloned().unwrap_or_default(),
            memo: memo_at.and_then(|at| header.get(at).cloned()),
        },
        rows,
    })
}

/// Rows as a file: `wallet,amount_usdc,memo`, a field quoted where it holds a comma, a quote or a
/// line break. What [`read()`] reads back to the same rows.
#[must_use]
pub fn write(rows: &[Row]) -> String {
    let mut out = String::from("wallet,amount_usdc,memo\n");
    for row in rows {
        let memo = row.memo.as_deref().unwrap_or("");
        out.push_str(&format!("{},{},{}\n", row.wallet.to_hex(), row.said(), quoted(memo)));
    }
    out
}

fn quoted(field: &str) -> String {
    match field.contains([',', '"', '\n', '\r']) {
        true => format!("\"{}\"", field.replace('"', "\"\"")),
        false => field.to_owned(),
    }
}

/// Each wallet and its amount in millionths, sorted: the same set whatever order it came in.
fn fingerprint(rows: &[Row]) -> String {
    let mut lines: Vec<String> =
        rows.iter().map(|row| format!("{}:{}", row.wallet.to_hex(), money::micros(row.amount).unwrap_or(0))).collect();
    lines.sort();
    lines.join("\n")
}

/// A header cell as a key: trimmed, lowercase, its spaces and dashes one underscore, quotes off.
fn normalise(cell: &str) -> String {
    let mut out = String::with_capacity(cell.len());
    let mut joining = false;
    for letter in cell.trim().chars() {
        if letter.is_whitespace() || letter == '-' {
            joining = true;
            continue;
        }
        if joining && !out.is_empty() {
            out.push('_');
        }
        joining = false;
        out.extend(letter.to_lowercase());
    }
    out.trim_matches('"').to_owned()
}

/// Lines into cells: a quoted cell may hold commas, line breaks and doubled quotes; a carriage
/// return is dropped.
fn split(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut letters = text.chars().peekable();
    while let Some(letter) = letters.next() {
        if quoted {
            match letter {
                '"' if letters.peek() == Some(&'"') => {
                    letters.next();
                    field.push('"');
                }
                '"' => quoted = false,
                other => field.push(other),
            }
            continue;
        }
        match letter {
            '"' => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' => {}
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
#[path = "../tests/csv.rs"]
mod tests;
