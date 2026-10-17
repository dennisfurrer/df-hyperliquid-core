//! `borrowLend`: `{type, operation, token, amount}`, keys in that order.

use crate::account::Signers;
use crate::account::signers::ActionKind;
use crate::utils::Dec;
use crate::utils::sign::{L1Action, Map, Value};

/// What a `borrowLend` does to a reserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Supply,
    Withdraw,
    Borrow,
    Repay,
}

impl Operation {
    pub const ALL: [Operation; 4] = [Operation::Supply, Operation::Withdraw, Operation::Borrow, Operation::Repay];

    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Operation::Supply => "supply",
            Operation::Withdraw => "withdraw",
            Operation::Borrow => "borrow",
            Operation::Repay => "repay",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadLend {
    #[error("{0} is not an amount above zero")]
    NotPositive(Dec),
}

/// `borrowLend`. `token` is the spot token's index from the caller's
/// `spotMeta`, a number, not its `NAME:0x..`: USDC is 0 on both networks
/// (`crate::client::USDC_INDEX`), HYPE 150 on mainnet and 1105 on testnet. An amount
/// of `None` goes as `null`: all of it, a whole withdraw or a whole repay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowLend {
    operation: Operation,
    token: u32,
    amount: Option<Dec>,
}

impl BorrowLend {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::BorrowLend.signers();

    /// `amount` in the token, already cut to its `weiDecimals`; it goes on
    /// the wire with no trailing zeros.
    pub fn new(operation: Operation, token: u32, amount: Dec) -> Result<BorrowLend, BadLend> {
        match amount.is_positive() {
            true => Ok(BorrowLend { operation, token, amount: Some(amount) }),
            false => Err(BadLend::NotPositive(amount)),
        }
    }

    pub fn supply(token: u32, amount: Dec) -> Result<BorrowLend, BadLend> {
        BorrowLend::new(Operation::Supply, token, amount)
    }

    pub fn withdraw(token: u32, amount: Dec) -> Result<BorrowLend, BadLend> {
        BorrowLend::new(Operation::Withdraw, token, amount)
    }

    pub fn borrow(token: u32, amount: Dec) -> Result<BorrowLend, BadLend> {
        BorrowLend::new(Operation::Borrow, token, amount)
    }

    pub fn repay(token: u32, amount: Dec) -> Result<BorrowLend, BadLend> {
        BorrowLend::new(Operation::Repay, token, amount)
    }

    /// Everything supplied, with what it earned.
    #[must_use]
    pub const fn withdraw_all(token: u32) -> BorrowLend {
        BorrowLend { operation: Operation::Withdraw, token, amount: None }
    }

    /// Everything owed, interest included.
    #[must_use]
    pub const fn repay_all(token: u32) -> BorrowLend {
        BorrowLend { operation: Operation::Repay, token, amount: None }
    }

    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    #[must_use]
    pub const fn token(&self) -> u32 {
        self.token
    }

    #[must_use]
    pub const fn amount(&self) -> Option<Dec> {
        self.amount
    }
}

impl L1Action for BorrowLend {
    fn action(&self) -> Map {
        let amount = match self.amount {
            Some(amount) => Value::from(amount.to_wire()),
            None => Value::Nil,
        };
        Map::new()
            .put("type", "borrowLend")
            .put("operation", self.operation.wire())
            .put("token", self.token)
            .put("amount", amount)
    }
}
