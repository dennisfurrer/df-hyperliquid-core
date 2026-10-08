//! `userNonFundingLedgerUpdates`: every movement of an account's money other
//! than funding. Each row's `delta` is tagged by `type`; a type this build
//! does not know reads as [`Delta::Other`] rather than failing the list.

use hl_core::{Address, ApiError, Dec, Json, Transport, info};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// `{"type":"userNonFundingLedgerUpdates","user":..,"startTime":..,"endTime"?:..}`,
/// times in milliseconds.
#[must_use]
pub fn ledger_request(user: Address, start_time: u64, end_time: Option<u64>) -> String {
    let mut request = vec![
        ("type".to_owned(), Json::String("userNonFundingLedgerUpdates".to_owned())),
        ("user".to_owned(), Json::String(user.to_hex())),
        ("startTime".to_owned(), Json::Number(start_time.to_string())),
    ];
    if let Some(end_time) = end_time {
        request.push(("endTime".to_owned(), Json::Number(end_time.to_string())));
    }
    Json::Object(request).compact()
}

/// The rows as the venue sends them, oldest first.
pub async fn ledger(
    transport: &impl Transport,
    user: Address,
    start_time: u64,
    end_time: Option<u64>,
) -> Result<Vec<LedgerUpdate>, ApiError> {
    info(transport, ledger_request(user, start_time, end_time)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LedgerUpdate {
    /// Milliseconds.
    pub time: u64,
    /// The L1 transaction hash, for the explorer.
    pub hash: String,
    #[serde(deserialize_with = "delta")]
    pub delta: Delta,
}

/// What a row did. USDC amounts named `usdc` are dollars; `amount` is in
/// `token`, which is the token's name (`"USDC"`, `"HYPE"`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Delta {
    /// `sendAsset`, from `user` to `destination`, possibly between balances.
    #[serde(rename_all = "camelCase")]
    Send {
        user: Address,
        destination: Address,
        #[serde(default)]
        source_dex: String,
        #[serde(default)]
        destination_dex: String,
        token: String,
        amount: Dec,
        #[serde(default)]
        usdc_value: Dec,
        #[serde(default)]
        fee: Dec,
        #[serde(default)]
        native_token_fee: Dec,
        #[serde(default)]
        nonce: Option<u64>,
        #[serde(default)]
        fee_token: String,
    },
    /// `usdSend`: USDC between perp balances.
    #[serde(rename_all = "camelCase")]
    InternalTransfer {
        usdc: Dec,
        user: Address,
        destination: Address,
        #[serde(default)]
        fee: Dec,
    },
    /// `spotSend`.
    #[serde(rename_all = "camelCase")]
    SpotTransfer {
        token: String,
        amount: Dec,
        #[serde(default)]
        usdc_value: Dec,
        user: Address,
        destination: Address,
        #[serde(default)]
        fee: Dec,
        #[serde(default)]
        native_token_fee: Dec,
        #[serde(default)]
        nonce: Option<u64>,
        #[serde(default)]
        fee_token: String,
    },
    /// In from the Arbitrum bridge.
    Deposit {
        usdc: Dec,
    },
    /// Out to Arbitrum, with the venue's fee.
    #[serde(rename_all = "camelCase")]
    Withdraw {
        usdc: Dec,
        nonce: u64,
        #[serde(default)]
        fee: Dec,
    },
    /// Between the account's own spot and perp balances.
    #[serde(rename_all = "camelCase")]
    AccountClassTransfer {
        usdc: Dec,
        to_perp: bool,
    },
    /// The fee a first transfer to a fresh address paid.
    AccountActivationGas {
        amount: Dec,
        token: String,
    },
    VaultDeposit {
        vault: Address,
        usdc: Dec,
    },
    #[serde(rename_all = "camelCase")]
    VaultWithdraw {
        vault: Address,
        user: Address,
        requested_usd: Dec,
        #[serde(default)]
        commission: Dec,
        #[serde(default)]
        closing_cost: Dec,
        #[serde(default)]
        basis: Dec,
        net_withdrawn_usd: Dec,
    },
    /// A type this build does not read, by name: `spotGenesis`,
    /// `cStakingTransfer`, `deployGasAuction`, and whatever comes next.
    #[serde(skip_deserializing)]
    Other {
        kind: String,
    },
}

impl Delta {
    /// The row's `type`, as the venue spells it.
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Delta::Send { .. } => "send",
            Delta::InternalTransfer { .. } => "internalTransfer",
            Delta::SpotTransfer { .. } => "spotTransfer",
            Delta::Deposit { .. } => "deposit",
            Delta::Withdraw { .. } => "withdraw",
            Delta::AccountClassTransfer { .. } => "accountClassTransfer",
            Delta::AccountActivationGas { .. } => "accountActivationGas",
            Delta::VaultDeposit { .. } => "vaultDeposit",
            Delta::VaultWithdraw { .. } => "vaultWithdraw",
            Delta::Other { kind } => kind,
        }
    }
}

/// A known type that no longer reads as modelled falls back to `Other` too:
/// one changed row must not blank the whole history.
fn delta<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Delta, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let kind = value.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
    Ok(serde_json::from_value(value).unwrap_or(Delta::Other { kind }))
}

#[cfg(test)]
#[path = "tests/ledger.rs"]
mod tests;
