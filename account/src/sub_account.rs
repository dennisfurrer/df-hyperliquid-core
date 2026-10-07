//! Subaccounts: `createSubAccount`, `subAccountTransfer` (perp USDC),
//! `subAccountSpotTransfer` (any spot token), and the `subAccounts` read.
//! All three actions are L1; hip4-tui signs them with the master key only,
//! since an agent's would open or drain the wrong account.

use hl_core::{Address, ApiError, BadUnits, Dec, L1Action, Map, Transport, exact_units, info, user_request};
use serde::Deserialize;

use crate::state::{PerpState, SpotState};

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
