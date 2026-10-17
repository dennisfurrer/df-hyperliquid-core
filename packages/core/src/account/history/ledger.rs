//! `userNonFundingLedgerUpdates`: every movement of an account's money other than funding, the
//! same rows the socket sends, and the read. A row type this build does not read, or one that no
//! longer reads as modelled, is [`Delta::Other`], kept as written.

use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Dec, Json};

/// `{"type":"userNonFundingLedgerUpdates","user":..,"startTime":..,"endTime"?:..}`,
/// times in milliseconds.
#[must_use]
pub fn ledger_request(user: Address, start_time: u64, end_time: Option<u64>) -> String {
    let mut fields = vec![("user", Json::String(user.to_hex())), ("startTime", Json::Number(start_time.to_string()))];
    if let Some(end_time) = end_time {
        fields.push(("endTime", Json::Number(end_time.to_string())));
    }
    info_request("userNonFundingLedgerUpdates", &fields)
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

/// The row a send signed with `nonce` made. Only `send` and `spotTransfer` rows carry their
/// action's nonce.
#[must_use]
pub fn find(rows: &[LedgerUpdate], nonce: u64) -> Option<&LedgerUpdate> {
    rows.iter().find(|row| {
        matches!(row.delta, Delta::Send { nonce: Some(said), .. } | Delta::SpotTransfer { nonce: Some(said), .. } if said == nonce)
    })
}

/// One ledger row.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LedgerUpdate {
    /// Milliseconds.
    pub time: u64,
    /// The L1 transaction hash, for the explorer.
    pub hash: String,
    #[serde(deserialize_with = "delta")]
    pub delta: Delta,
}

impl LedgerUpdate {
    /// The row's `type`: `deposit`, `send`, `withdraw`, `accountClassTransfer` and the rest.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.delta.kind()
    }
}

/// What a row did. USDC amounts named `usdc` are dollars; `amount` is in `token`, the token's name
/// (`"USDC"`, `"HYPE"`). A field not every row of its type carries is an `Option`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Delta {
    /// `sendAsset`, from `user` to `destination`, possibly between balances.
    #[serde(rename_all = "camelCase")]
    Send {
        user: Address,
        destination: Address,
        /// `""` the main perp dex, `spot` spot.
        source_dex: String,
        destination_dex: String,
        token: String,
        amount: Dec,
        usdc_value: Dec,
        fee: Dec,
        /// Not always sent.
        #[serde(default)]
        native_token_fee: Option<Dec>,
        #[serde(default)]
        nonce: Option<u64>,
        /// `""` where no fee was charged.
        fee_token: String,
    },
    /// `usdSend`: USDC between perp balances.
    #[serde(rename_all = "camelCase")]
    InternalTransfer {
        usdc: Dec,
        user: Address,
        destination: Address,
        fee: Dec,
    },
    /// `spotSend`.
    #[serde(rename_all = "camelCase")]
    SpotTransfer {
        token: String,
        amount: Dec,
        usdc_value: Dec,
        user: Address,
        destination: Address,
        fee: Dec,
        /// Not always sent.
        #[serde(default)]
        native_token_fee: Option<Dec>,
        #[serde(default)]
        nonce: Option<u64>,
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
        commission: Dec,
        closing_cost: Dec,
        basis: Dec,
        net_withdrawn_usd: Dec,
    },
    /// HYPE between spot and the staking balance: `cDeposit` in, `cWithdraw` out once its queue
    /// has passed.
    #[serde(rename_all = "camelCase")]
    CStakingTransfer {
        token: String,
        amount: Dec,
        is_deposit: bool,
    },
    /// A deployer's share of a token it launched.
    SpotGenesis {
        token: String,
        amount: Dec,
    },
    /// A bid in a deploy gas auction.
    DeployGasAuction {
        token: String,
        amount: Dec,
    },
    /// Referral or builder rewards claimed: an amount of `token`, not `usdc`.
    RewardsClaim {
        amount: Dec,
        token: String,
    },
    /// USDC between a master and one of its subaccounts, `user` to `destination`.
    SubAccountTransfer {
        usdc: Dec,
        user: Address,
        destination: Address,
    },
    /// A vault made: the USDC its leader put in, and the creation fee charged beside it.
    VaultCreate {
        vault: Address,
        usdc: Dec,
        fee: Dec,
    },
    /// A type this build does not read, or a known one that no longer reads as modelled: its
    /// `type` (empty when it names none) and the delta as written, every key and digit.
    #[serde(skip_deserializing)]
    Other {
        kind: String,
        fields: Json,
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
            Delta::CStakingTransfer { .. } => "cStakingTransfer",
            Delta::SpotGenesis { .. } => "spotGenesis",
            Delta::DeployGasAuction { .. } => "deployGasAuction",
            Delta::RewardsClaim { .. } => "rewardsClaim",
            Delta::SubAccountTransfer { .. } => "subAccountTransfer",
            Delta::VaultCreate { .. } => "vaultCreate",
            Delta::Other { kind, .. } => kind,
        }
    }

    /// The nonce the row carries, as the venue writes it. A send's is its action's; a withdrawal's
    /// runs to sixteen digits (`1759909832650000`) and is not.
    #[must_use]
    pub const fn nonce(&self) -> Option<u64> {
        match self {
            Delta::Send { nonce, .. } | Delta::SpotTransfer { nonce, .. } => *nonce,
            Delta::Withdraw { nonce, .. } => Some(*nonce),
            _ => None,
        }
    }
}

/// Read where it lies, never through a float: a known type as modelled, anything else kept as
/// written. One changed row must not blank the whole history.
fn delta<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Delta, D::Error> {
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    if let Ok(delta) = serde_json::from_str::<Delta>(raw.get()) {
        return Ok(delta);
    }
    let fields = Json::parse(raw.get()).map_err(de::Error::custom)?;
    let kind = fields.get("type").and_then(Json::as_str).unwrap_or_default().to_owned();
    Ok(Delta::Other { kind, fields })
}

#[cfg(test)]
#[path = "tests/ledger.rs"]
mod tests;
