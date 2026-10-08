//! `userNonFundingLedgerUpdates`: every movement of an account's money other than funding, the
//! same rows whether `/info` answers them or the socket sends them. Each row's `delta` is tagged
//! by `type`; a type this build does not read, or a known one that no longer reads as modelled,
//! is [`Delta::Other`], kept as written, rather than failing the list or reading as zero.

use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::{address::Address, dec::Dec, json::Json};

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

/// What a row did. USDC amounts named `usdc` are dollars; `amount` is in `token`, which is the
/// token's name (`"USDC"`, `"HYPE"`). Every field is one each recorded row of its type carries;
/// one that only some carry is an `Option`.
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
        /// Left out of hip4-tui's recorded rows.
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
        /// Left out of hip4-tui's recorded rows.
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
            Delta::Other { kind, .. } => kind,
        }
    }

    /// The nonce the row carries, as the venue writes it. A send's is its action's; a
    /// withdrawal's runs to sixteen digits (live: `1759909832650000`) and is not.
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
