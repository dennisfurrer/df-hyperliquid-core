//! `cDeposit`, `cWithdraw` and `tokenDelegate`: user-signed, keys in the order
//! `@nktkas/hyperliquid` 0.33.2 posts them. `cDeposit2`: the L1 form of `cDeposit`, as the exchange
//! endpoint docs write it.

use crate::account::{ActionKind, Signers};
use crate::utils::sign::{Field, L1Action, Map, NonceKey, UserAction, UserSigned};
use crate::utils::{Address, BadUnits, Dec, HYPE_DECIMALS, exact_units};

/// `hype` as the `wei` an action carries, refused rather than rounded when
/// it has more than eight places.
pub fn hype_wei(hype: Dec) -> Result<u64, BadUnits> {
    exact_units(hype, HYPE_DECIMALS)
}

/// `cDeposit`: HYPE from spot into the staking balance, undelegated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CDeposit {
    wei: u64,
    nonce: u64,
}

impl CDeposit {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::CDeposit.signers();

    pub fn new(hype: Dec, nonce: u64) -> Result<CDeposit, BadUnits> {
        Ok(CDeposit { wei: hype_wei(hype)?, nonce })
    }

    #[must_use]
    pub const fn wei(&self) -> u64 {
        self.wei
    }
}

impl UserAction for CDeposit {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("cDeposit", "HyperliquidTransaction:CDeposit", NonceKey::Nonce, self.nonce)
            .field("wei", Field::Uint64(self.wei))
    }
}

/// `cDeposit2`: [`CDeposit`] as an L1 action, `{type, wei}`, which an agent may sign for its
/// master. It carries no nonce of its own: the one it is signed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CDeposit2 {
    wei: u64,
}

impl CDeposit2 {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::CDeposit2.signers();

    /// `hype` from spot into the staking balance, refused rather than rounded past eight places.
    pub fn new(hype: Dec) -> Result<CDeposit2, BadUnits> {
        Ok(CDeposit2 { wei: hype_wei(hype)? })
    }

    /// The amount as the action carries it, in hundred-millionths of HYPE.
    #[must_use]
    pub const fn wei(&self) -> u64 {
        self.wei
    }
}

impl L1Action for CDeposit2 {
    fn action(&self) -> Map {
        Map::new().put("type", "cDeposit2").put("wei", self.wei)
    }
}

/// `cWithdraw`: undelegated HYPE from staking back toward spot, after
/// [`unstaking_ms`](crate::earn::staking::unstaking_ms) in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CWithdraw {
    wei: u64,
    nonce: u64,
}

impl CWithdraw {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::CWithdraw.signers();

    pub fn new(hype: Dec, nonce: u64) -> Result<CWithdraw, BadUnits> {
        Ok(CWithdraw { wei: hype_wei(hype)?, nonce })
    }

    #[must_use]
    pub const fn wei(&self) -> u64 {
        self.wei
    }
}

impl UserAction for CWithdraw {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("cWithdraw", "HyperliquidTransaction:CWithdraw", NonceKey::Nonce, self.nonce)
            .field("wei", Field::Uint64(self.wei))
    }
}

/// `tokenDelegate`: staking balance to a validator, or back. A delegation is locked for
/// [`delegation_lock_ms`](crate::earn::staking::delegation_lock_ms) before it can be undelegated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenDelegate {
    validator: Address,
    wei: u64,
    is_undelegate: bool,
    nonce: u64,
}

impl TokenDelegate {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::TokenDelegate.signers();

    pub fn delegate(validator: Address, hype: Dec, nonce: u64) -> Result<TokenDelegate, BadUnits> {
        Ok(TokenDelegate { validator, wei: hype_wei(hype)?, is_undelegate: false, nonce })
    }

    pub fn undelegate(validator: Address, hype: Dec, nonce: u64) -> Result<TokenDelegate, BadUnits> {
        Ok(TokenDelegate { validator, wei: hype_wei(hype)?, is_undelegate: true, nonce })
    }

    #[must_use]
    pub const fn wei(&self) -> u64 {
        self.wei
    }
}

/// `validator` is an `address` in the typed data, posted lowercase.
impl UserAction for TokenDelegate {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("tokenDelegate", "HyperliquidTransaction:TokenDelegate", NonceKey::Nonce, self.nonce)
            .field("validator", Field::Address(self.validator))
            .field("wei", Field::Uint64(self.wei))
            .field("isUndelegate", Field::Bool(self.is_undelegate))
    }
}
