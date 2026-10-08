//! The guards a person sets on payouts, as the payout tool's `config.json` holds them, key for
//! key. Off until armed: a config that says nothing is a dry run with caps on a batch and a file.
//! A cap that will not read is a problem, never no cap.

use hl_core::Dec;
use serde::{Deserialize, Serialize};

use super::money;

/// One guard, by its key in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    Live,
    AllowFreshAccounts,
    MinPayout,
    MaxSinglePayout,
    MaxBatchTotal,
    MinHypeBalance,
    MaxCsvTotal,
    SendPacing,
    MaxAttempts,
    DuplicateWindow,
}

impl Guard {
    pub const ALL: [Guard; 10] = [
        Guard::Live,
        Guard::AllowFreshAccounts,
        Guard::MinPayout,
        Guard::MaxSinglePayout,
        Guard::MaxBatchTotal,
        Guard::MinHypeBalance,
        Guard::MaxCsvTotal,
        Guard::SendPacing,
        Guard::MaxAttempts,
        Guard::DuplicateWindow,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Guard::Live => "live",
            Guard::AllowFreshAccounts => "allowFreshAccounts",
            Guard::MinPayout => "minPayoutUsdc",
            Guard::MaxSinglePayout => "maxSinglePayoutUsdc",
            Guard::MaxBatchTotal => "maxBatchTotalUsdc",
            Guard::MinHypeBalance => "minHypeBalance",
            Guard::MaxCsvTotal => "maxCsvTotalUsdc",
            Guard::SendPacing => "sendPacingMs",
            Guard::MaxAttempts => "maxAttempts",
            Guard::DuplicateWindow => "duplicateWindowDays",
        }
    }

    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Guard::Live => "off means every send is a dry run",
            Guard::AllowFreshAccounts => "off parks an address that has never touched the venue",
            Guard::MinPayout => "skips a row below this instead of sending dust",
            Guard::MaxSinglePayout => "parks a row above this for review",
            Guard::MaxBatchTotal => "the most value that may move between two balance checks",
            Guard::MinHypeBalance => "refuses to send unless the treasury holds this much HYPE",
            Guard::MaxCsvTotal => "refuses a whole file above this",
            Guard::SendPacing => "milliseconds between sends, to stay under the rate limit",
            Guard::MaxAttempts => "attempts per row before it stops being selected",
            Guard::DuplicateWindow => "days back to look for the same wallet paid the same amount",
        }
    }

    /// The guards that are amounts.
    const AMOUNTS: [Guard; 5] =
        [Guard::MinPayout, Guard::MaxSinglePayout, Guard::MaxBatchTotal, Guard::MinHypeBalance, Guard::MaxCsvTotal];
}

/// The guards. Amounts are kept as typed, and read when used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The master switch. Off means every send is a dry run, whatever else is set.
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub allow_fresh_accounts: bool,
    #[serde(default)]
    pub min_payout_usdc: Option<String>,
    #[serde(default)]
    pub max_single_payout_usdc: Option<String>,
    #[serde(default = "batch_total")]
    pub max_batch_total_usdc: Option<String>,
    #[serde(default)]
    pub min_hype_balance: Option<String>,
    #[serde(default = "csv_total")]
    pub max_csv_total_usdc: Option<String>,
    #[serde(default = "pacing")]
    pub send_pacing_ms: u64,
    #[serde(default = "attempts")]
    pub max_attempts: u32,
    #[serde(default = "window")]
    pub duplicate_window_days: i64,
}

fn batch_total() -> Option<String> {
    Some("5000".to_owned())
}

fn csv_total() -> Option<String> {
    Some("20000".to_owned())
}

const fn pacing() -> u64 {
    120
}

const fn attempts() -> u32 {
    5
}

const fn window() -> i64 {
    7
}

impl Default for Config {
    fn default() -> Self {
        Config {
            live: false,
            allow_fresh_accounts: false,
            min_payout_usdc: None,
            max_single_payout_usdc: None,
            max_batch_total_usdc: batch_total(),
            min_hype_balance: None,
            max_csv_total_usdc: csv_total(),
            send_pacing_ms: pacing(),
            max_attempts: attempts(),
            duplicate_window_days: window(),
        }
    }
}

impl Config {
    /// The file's text; an empty file is the default, as a file not written yet is.
    pub fn read(text: &str) -> Result<Config, serde_json::Error> {
        match text.trim().is_empty() {
            true => Ok(Config::default()),
            false => serde_json::from_str(text),
        }
    }

    /// The file's text, pretty and ending its last line.
    #[must_use]
    pub fn write(&self) -> String {
        let body = serde_json::to_string_pretty(self).unwrap_or_default();
        format!("{body}\n")
    }

    /// What a guard is set to, in words.
    #[must_use]
    pub fn says(&self, guard: Guard) -> String {
        let cap = |value: Option<&str>| value.unwrap_or("none").to_owned();
        match guard {
            Guard::Live if self.live => "armed".to_owned(),
            Guard::Live => "dry run".to_owned(),
            Guard::AllowFreshAccounts if self.allow_fresh_accounts => "allowed".to_owned(),
            Guard::AllowFreshAccounts => "parked".to_owned(),
            Guard::SendPacing => format!("{} ms", self.send_pacing_ms),
            Guard::MaxAttempts => self.max_attempts.to_string(),
            Guard::DuplicateWindow => format!("{} days", self.duplicate_window_days),
            amount => cap(self.typed(amount)),
        }
    }

    /// An amount guard as typed.
    fn typed(&self, guard: Guard) -> Option<&str> {
        match guard {
            Guard::MinPayout => self.min_payout_usdc.as_deref(),
            Guard::MaxSinglePayout => self.max_single_payout_usdc.as_deref(),
            Guard::MaxBatchTotal => self.max_batch_total_usdc.as_deref(),
            Guard::MinHypeBalance => self.min_hype_balance.as_deref(),
            Guard::MaxCsvTotal => self.max_csv_total_usdc.as_deref(),
            _ => None,
        }
    }

    /// An amount guard's figure; `None` unset, and for one that will not read.
    #[must_use]
    pub fn limit(&self, guard: Guard) -> Option<Dec> {
        money::parse(self.typed(guard)?).ok()
    }

    /// Everything wrong with the guards. A cap that will not read must never pass for no cap.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems: Vec<String> = Guard::AMOUNTS
            .into_iter()
            .filter_map(|guard| {
                let said = self.typed(guard)?;
                money::parse(said).is_err().then(|| format!("{}: `{said}` is not an amount", guard.key()))
            })
            .collect();
        if self.max_attempts < 1 {
            problems.push("maxAttempts must be at least one".to_owned());
        }
        problems
    }

    /// A limit as typed: cleared by nothing, `none`, `null`, `off` or `-`; kept as typed once it
    /// reads as an amount.
    pub fn read_limit(typed: &str) -> Result<Option<String>, money::BadUsdc> {
        let said = typed.trim();
        if matches!(said.to_ascii_lowercase().as_str(), "" | "none" | "null" | "off" | "-") {
            return Ok(None);
        }
        money::parse(said)?;
        Ok(Some(said.to_owned()))
    }
}

#[cfg(test)]
#[path = "../tests/config.rs"]
mod tests;
