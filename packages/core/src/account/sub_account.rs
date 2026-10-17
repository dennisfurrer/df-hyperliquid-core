//! Subaccounts: `createSubAccount`, `subAccountTransfer` (perp USDC), `subAccountSpotTransfer` (any
//! spot token) and the `subAccounts` read. A subaccount is a user of its own with no key: it is read
//! at its own address, and the master's agent trades its perps with it as `vaultAddress`.

use std::fmt;

use crate::account::signers::ActionKind;
use crate::account::state::{PerpState, SpotState};
use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Address, AssetKind, BadUnits, Dec, exact_units};
use serde::Deserialize;

/// Whose account an action or a read is about: the master's own, or one of
/// its subaccounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountRef {
    /// The account whose key, or whose agent, signs.
    pub owner: Address,
    /// The subaccount acted for, if any.
    pub sub: Option<Address>,
}

/// What a subaccount does not do, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum SubRefusal {
    #[error("a subaccount places no spot orders here")]
    Spot,
    #[error("a subaccount places no HIP-4 orders and splits or merges nothing here")]
    Outcome,
    /// An order-like action that names no market cannot be told apart.
    #[error("a subaccount's order names its market, since its spot and HIP-4 orders are refused")]
    MarketUnknown,
    #[error("a subaccount moves nothing between its own ledgers here")]
    LedgerMove,
    #[error("a subaccount approves nothing: its master's agent signs for it")]
    Approval,
    #[error("a subaccount schedules no cancel here")]
    ScheduleCancel,
    /// Opening a subaccount, and moving money in and out of one, is the
    /// master's own business.
    #[error("only the master opens subaccounts and moves money in and out of them")]
    MastersOwn,
    /// User-signed: it would need the subaccount's own key, which there is not.
    #[error("a subaccount has no key of its own to sign this")]
    NoKey,
}

impl AccountRef {
    #[must_use]
    pub const fn own(owner: Address) -> AccountRef {
        AccountRef { owner, sub: None }
    }

    #[must_use]
    pub const fn sub(owner: Address, sub: Address) -> AccountRef {
        AccountRef { owner, sub: Some(sub) }
    }

    #[must_use]
    pub const fn is_sub(&self) -> bool {
        self.sub.is_some()
    }

    /// Where its states, mode, fees and limits are read: the subaccount's
    /// own address for one.
    #[must_use]
    pub fn read_at(&self) -> Address {
        self.sub.unwrap_or(self.owner)
    }

    /// The `vaultAddress` an L1 action names: the subaccount it acts for.
    #[must_use]
    pub const fn vault_address(&self) -> Option<Address> {
        self.sub
    }

    /// The account whose key, or whose agent, signs.
    #[must_use]
    pub const fn signs_for(&self) -> Address {
        self.owner
    }

    /// Why this account does not do `kind`, or `None` when it may. An
    /// order-like action (`order`, `modify`, `batchModify`, `twapOrder`) names
    /// its `market`. The master's own account refuses nothing here.
    #[must_use]
    pub fn refuses(&self, kind: ActionKind, market: Option<AssetKind>) -> Option<SubRefusal> {
        self.sub?;
        match kind {
            ActionKind::Order | ActionKind::Modify | ActionKind::BatchModify | ActionKind::TwapOrder => match market {
                Some(AssetKind::Perp | AssetKind::Hip3Perp) => None,
                Some(AssetKind::Spot) => Some(SubRefusal::Spot),
                Some(AssetKind::Outcome) => Some(SubRefusal::Outcome),
                None => Some(SubRefusal::MarketUnknown),
            },
            ActionKind::UserOutcome => Some(SubRefusal::Outcome),
            ActionKind::UsdClassTransfer | ActionKind::AgentSendAsset => Some(SubRefusal::LedgerMove),
            ActionKind::ApproveAgent | ActionKind::ApproveBuilderFee => Some(SubRefusal::Approval),
            ActionKind::ScheduleCancel => Some(SubRefusal::ScheduleCancel),
            ActionKind::CreateSubAccount | ActionKind::SubAccountTransfer | ActionKind::SubAccountSpotTransfer => {
                Some(SubRefusal::MastersOwn)
            }
            // The docs: `userSetAbstraction`'s user "can be a sub-account of the user".
            ActionKind::UserSetAbstraction => None,
            user_signed if user_signed.is_user_signed() => Some(SubRefusal::NoKey),
            _ => None,
        }
    }
}

impl fmt::Display for AccountRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.sub {
            Some(sub) => write!(f, "{sub} (a subaccount of {})", self.owner),
            None => write!(f, "{}", self.owner),
        }
    }
}

/// `createSubAccount`: `{type, name}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSubAccount {
    name: String,
}

impl CreateSubAccount {
    #[must_use]
    pub fn new(name: &str) -> CreateSubAccount {
        CreateSubAccount { name: name.to_owned() }
    }
}

impl L1Action for CreateSubAccount {
    fn action(&self) -> Map {
        Map::new().put("type", "createSubAccount").put("name", self.name.clone())
    }
}

/// `subAccountTransfer`: perp USDC between the master and a subaccount,
/// `usd` in millionths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubAccountTransfer {
    sub_account: Address,
    is_deposit: bool,
    usd: u64,
}

impl SubAccountTransfer {
    /// Into the subaccount, at most six places: refused rather than rounded
    /// when finer.
    pub fn deposit(sub_account: Address, usdc: Dec) -> Result<SubAccountTransfer, BadUnits> {
        Ok(SubAccountTransfer { sub_account, is_deposit: true, usd: exact_units(usdc, 6)? })
    }

    /// Back to the master.
    pub fn withdraw(sub_account: Address, usdc: Dec) -> Result<SubAccountTransfer, BadUnits> {
        Ok(SubAccountTransfer { sub_account, is_deposit: false, usd: exact_units(usdc, 6)? })
    }

    /// From a count of millionths already made.
    #[must_use]
    pub const fn from_micros(sub_account: Address, is_deposit: bool, usd: u64) -> SubAccountTransfer {
        SubAccountTransfer { sub_account, is_deposit, usd }
    }
}

impl L1Action for SubAccountTransfer {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "subAccountTransfer")
            .put("subAccountUser", self.sub_account.to_hex())
            .put("isDeposit", self.is_deposit)
            .put("usd", self.usd)
    }
}

/// `subAccountSpotTransfer`: a spot token between the master and a
/// subaccount. `token` is `NAME:0x<tokenId>`; the amount goes as a decimal
/// with no trailing zeros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAccountSpotTransfer {
    sub_account: Address,
    is_deposit: bool,
    token: String,
    amount: Dec,
}

impl SubAccountSpotTransfer {
    pub fn new(
        sub_account: Address,
        is_deposit: bool,
        token: &str,
        amount: Dec,
    ) -> Result<SubAccountSpotTransfer, BadUnits> {
        if !amount.is_positive() {
            return Err(BadUnits::NotPositive(amount));
        }
        Ok(SubAccountSpotTransfer { sub_account, is_deposit, token: token.to_owned(), amount })
    }
}

impl L1Action for SubAccountSpotTransfer {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "subAccountSpotTransfer")
            .put("subAccountUser", self.sub_account.to_hex())
            .put("isDeposit", self.is_deposit)
            .put("token", self.token.clone())
            .put("amount", self.amount.to_wire())
    }
}

/// `{"type":"subAccounts","user":<master>}`.
#[must_use]
pub fn sub_accounts_request(master: Address) -> String {
    user_request("subAccounts", master)
}

/// A master's subaccounts. The venue answers `null`, not `[]`, for none.
pub async fn sub_accounts(transport: &impl Transport, master: Address) -> Result<Vec<SubAccount>, ApiError> {
    let subs: Option<Vec<SubAccount>> = info(transport, sub_accounts_request(master)).await?;
    Ok(subs.unwrap_or_default())
}

/// One subaccount with its states. A subaccount is a user in its own right,
/// so its figures are never added to its master's.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAccount {
    #[serde(default)]
    pub name: String,
    pub sub_account_user: Address,
    pub master: Address,
    #[serde(default)]
    pub clearinghouse_state: Option<PerpState>,
    #[serde(default)]
    pub spot_state: Option<SpotState>,
}

impl SubAccount {
    /// Acting for it, as its master.
    #[must_use]
    pub const fn account(&self) -> AccountRef {
        AccountRef::sub(self.master, self.sub_account_user)
    }

    /// Its main dex's account value; nothing when it has no state.
    #[must_use]
    pub fn equity(&self) -> Dec {
        self.clearinghouse_state.as_ref().map_or(Dec::ZERO, |state| state.margin_summary.account_value)
    }

    #[must_use]
    pub fn notional(&self) -> Dec {
        self.clearinghouse_state.as_ref().map_or(Dec::ZERO, |state| state.margin_summary.total_ntl_pos)
    }

    #[must_use]
    pub fn withdrawable(&self) -> Dec {
        self.clearinghouse_state.as_ref().map_or(Dec::ZERO, |state| state.withdrawable)
    }

    /// Positions held on its main dex, a flat leg left out.
    #[must_use]
    pub fn positions(&self) -> usize {
        self.clearinghouse_state.as_ref().map_or(0, |state| state.positions().count())
    }

    /// Spot USDC's total.
    #[must_use]
    pub fn spot_usdc(&self) -> Dec {
        self.spot_state.as_ref().and_then(SpotState::usdc).map_or(Dec::ZERO, |usdc| usdc.total)
    }
}

#[cfg(test)]
#[path = "tests/sub_account.rs"]
mod tests;
