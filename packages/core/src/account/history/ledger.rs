//! `userNonFundingLedgerUpdates`: every movement of an account's money other than funding, the
//! same rows the socket sends, and the read. A row type this build does not read, or one that no
//! longer reads as modelled, is [`Delta::Other`], kept as written.

use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::client::{ApiError, Transport, info, info_request};
use crate::earn::lending::Operation;
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
#[non_exhaustive]
pub struct LedgerUpdate {
    /// Milliseconds.
    pub time: u64,
    /// The L1 transaction hash, for the explorer.
    pub hash: String,
    /// What the row did; a row that does not read as modelled is [`Delta::Other`].
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
#[non_exhaustive]
pub enum Delta {
    /// `sendAsset`, from `user` to `destination`, possibly between balances.
    #[serde(rename_all = "camelCase")]
    Send {
        /// The sender.
        user: Address,
        /// The receiver; the sender's own address for a move between its own balances.
        destination: Address,
        /// `""` the main perp dex, `spot` spot.
        source_dex: String,
        /// The dex the amount went to (`destinationDex`): `""` the main perp dex, `spot` spot.
        destination_dex: String,
        /// The token's name, `USDC` or `HYPE`.
        token: String,
        /// How much of `token` moved.
        amount: Dec,
        /// What the amount was worth when it moved (`usdcValue`), in USDC.
        usdc_value: Dec,
        /// The fee charged, in `fee_token`; zero when none.
        fee: Dec,
        /// Not always sent.
        #[serde(default)]
        native_token_fee: Option<Dec>,
        /// The nonce of the action that made the row; `None` when the venue leaves it out.
        #[serde(default)]
        nonce: Option<u64>,
        /// `""` where no fee was charged.
        fee_token: String,
    },
    /// `usdSend`: USDC between perp balances.
    #[serde(rename_all = "camelCase")]
    InternalTransfer {
        /// How much moved, in USDC.
        usdc: Dec,
        /// The sender.
        user: Address,
        /// The receiver.
        destination: Address,
        /// The fee charged, in USDC.
        fee: Dec,
    },
    /// `spotSend`.
    #[serde(rename_all = "camelCase")]
    SpotTransfer {
        /// The token's name.
        token: String,
        /// How much of `token` moved.
        amount: Dec,
        /// What the amount was worth when it moved (`usdcValue`), in USDC.
        usdc_value: Dec,
        /// The sender.
        user: Address,
        /// The receiver.
        destination: Address,
        /// The fee charged, in `fee_token`; zero when none.
        fee: Dec,
        /// Not always sent.
        #[serde(default)]
        native_token_fee: Option<Dec>,
        /// The nonce of the `spotSend` that made the row; `None` when the venue leaves it out.
        #[serde(default)]
        nonce: Option<u64>,
        /// The token the fee was paid in (`feeToken`); `""` where no fee was charged.
        fee_token: String,
    },
    /// In from the Arbitrum bridge.
    Deposit {
        /// How much came in, in USDC.
        usdc: Dec,
    },
    /// Out to Arbitrum, with the venue's fee.
    #[serde(rename_all = "camelCase")]
    Withdraw {
        /// How much left the account, in USDC.
        usdc: Dec,
        /// The withdrawal's nonce as the venue writes it, sixteen digits; not the signed action's
        /// nonce.
        nonce: u64,
        /// The venue's fee, in USDC.
        fee: Dec,
    },
    /// Between the account's own spot and perp balances.
    #[serde(rename_all = "camelCase")]
    AccountClassTransfer {
        /// How much moved, in USDC.
        usdc: Dec,
        /// `true` from spot to perps, `false` from perps to spot (`toPerp`).
        to_perp: bool,
    },
    /// The fee a first transfer to a fresh address paid.
    AccountActivationGas {
        /// How much was charged, in `token`.
        amount: Dec,
        /// The token the fee was paid in.
        token: String,
    },
    /// USDC put into a vault.
    VaultDeposit {
        /// The vault's address.
        vault: Address,
        /// How much went in, in USDC.
        usdc: Dec,
    },
    /// USDC taken out of a vault, net of the leader's commission and the cost of closing positions.
    #[serde(rename_all = "camelCase")]
    VaultWithdraw {
        /// The vault's address.
        vault: Address,
        /// The depositor who withdrew.
        user: Address,
        /// How much the depositor asked for (`requestedUsd`), in USDC.
        requested_usd: Dec,
        /// The share of the profit the vault's leader took, in USDC.
        commission: Dec,
        /// The cost of closing positions to pay the withdrawal out (`closingCost`), in USDC.
        closing_cost: Dec,
        /// What the withdrawn part of the deposit cost the depositor, in USDC.
        basis: Dec,
        /// What the depositor received (`netWithdrawnUsd`), in USDC.
        net_withdrawn_usd: Dec,
    },
    /// HYPE between spot and the staking balance: `cDeposit` in, `cWithdraw` out once its queue
    /// has passed.
    #[serde(rename_all = "camelCase")]
    CStakingTransfer {
        /// The token's name, `HYPE`.
        token: String,
        /// How much moved, in `token`.
        amount: Dec,
        /// `true` into staking (`cDeposit`), `false` out of it (`cWithdraw`) (`isDeposit`).
        is_deposit: bool,
    },
    /// A deployer's share of a token it launched.
    SpotGenesis {
        /// The token's name.
        token: String,
        /// How much the deployer received, in `token`.
        amount: Dec,
    },
    /// A bid in a deploy gas auction.
    DeployGasAuction {
        /// The token the bid was paid in, `HYPE`.
        token: String,
        /// The bid, in `token`.
        amount: Dec,
    },
    /// Referral or builder rewards claimed: an amount of `token`, not `usdc`.
    RewardsClaim {
        /// How much was claimed, in `token`.
        amount: Dec,
        /// The token claimed in.
        token: String,
    },
    /// USDC between a master and one of its subaccounts, `user` to `destination`.
    SubAccountTransfer {
        /// How much moved, in USDC.
        usdc: Dec,
        /// The sender.
        user: Address,
        /// The receiver.
        destination: Address,
    },
    /// A vault made: the USDC its leader put in, and the creation fee charged beside it.
    VaultCreate {
        /// The new vault's address.
        vault: Address,
        /// What the leader put in, in USDC.
        usdc: Dec,
        /// The creation fee, in USDC.
        fee: Dec,
    },
    /// A supply, withdrawal, borrow or repayment on a lending reserve: `amount` of `token`, of which
    /// `interest_amount` is interest earned or paid.
    #[serde(rename_all = "camelCase")]
    BorrowLend {
        /// The token's name.
        token: String,
        /// What was done: supply, withdraw, borrow or repay.
        operation: Operation,
        /// How much of `token` moved.
        amount: Dec,
        /// The part of `amount` that is interest earned or paid (`interestAmount`).
        interest_amount: Dec,
    },
    /// The account was liquidated: the notional taken over and what the account was worth then.
    #[serde(rename_all = "camelCase")]
    Liquidation {
        /// The notional of the positions taken over (`liquidatedNtlPos`), in USDC.
        liquidated_ntl_pos: Dec,
        /// What the account was worth at the liquidation (`accountValue`), in USDC.
        account_value: Dec,
        /// `Cross` or `Isolated`, capitalized here unlike anywhere else.
        leverage_type: String,
        /// The positions taken over (`liquidatedPositions`).
        liquidated_positions: Vec<LiquidatedPosition>,
    },
    /// A type this build does not read, or a known one that no longer reads as modelled: its
    /// `type` (empty when it names none) and the delta as written, every key and digit.
    #[serde(skip_deserializing)]
    Other {
        /// The row's `type`; empty when it names none.
        kind: String,
        /// The delta as written, every key and digit.
        fields: Json,
    },
}

/// A position a liquidation took over: its coin and signed size.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct LiquidatedPosition {
    /// The coin, `ATOM` or `xyz:XYZ100`.
    pub coin: String,
    /// The size taken over, signed: negative for a short.
    pub szi: Dec,
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
            Delta::BorrowLend { .. } => "borrowLend",
            Delta::Liquidation { .. } => "liquidation",
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
