//! Lending: one account's borrow and supply in a token, and a token's pool.

use crate::client::evm::abi::AbiError;
use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Address;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasisAndValue {
    pub basis: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowLendUserState {
    pub borrow: BasisAndValue,
    pub supply: BasisAndValue,
}

/// `0x811`: one account's borrow and supply in a token. The precompile
/// refuses an account that has never lent or borrowed it.
#[must_use]
pub fn borrow_lend_user_state(user: Address, token: u64) -> Read<BorrowLendUserState> {
    read(0x811, &[Arg::Address(user), Arg::Uint(token)], |answer| {
        let tokens = tokens(0x811, answer)?;
        let mut fields = Fields::of("borrowLendUserState", &tokens)?;
        let pair = |fields: &mut Fields<'_>| -> Result<BasisAndValue, AbiError> {
            let item = fields.items.next().ok_or_else(|| AbiError::Type("borrowLendUserState: short".to_owned()))?;
            let mut inner = Fields::of("borrowLendUserState", std::slice::from_ref(item))?;
            Ok(BasisAndValue { basis: inner.u64()?, value: inner.u64()? })
        };
        Ok(BorrowLendUserState { borrow: pair(&mut fields)?, supply: pair(&mut fields)? })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowLendReserveState {
    pub borrow_yearly_rate_bps: u64,
    pub supply_yearly_rate_bps: u64,
    pub balance: u64,
    pub utilization_bps: u64,
    pub oracle_px: u64,
    pub ltv_bps: u64,
    pub total_supplied: u64,
    pub total_borrowed: u64,
}

/// `0x812`: a token's lending pool. Its token is a `uint64`, where most
/// neighbours take a `uint32`.
#[must_use]
pub fn borrow_lend_reserve_state(token: u64) -> Read<BorrowLendReserveState> {
    read(0x812, &[Arg::Uint(token)], |answer| {
        let tokens = tokens(0x812, answer)?;
        let mut fields = Fields::of("borrowLendReserveState", &tokens)?;
        Ok(BorrowLendReserveState {
            borrow_yearly_rate_bps: fields.u64()?,
            supply_yearly_rate_bps: fields.u64()?,
            balance: fields.u64()?,
            utilization_bps: fields.u64()?,
            oracle_px: fields.u64()?,
            ltv_bps: fields.u64()?,
            total_supplied: fields.u64()?,
            total_borrowed: fields.u64()?,
        })
    })
}

#[cfg(test)]
#[path = "tests/lending.rs"]
mod tests;
