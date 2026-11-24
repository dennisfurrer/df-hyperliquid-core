//! `convertToMultiSigUser`: turns an account into a multi-sig account, changes its signers, or
//! turns it back into a normal account. User-signed: `signers` is a JSON text, signed as a string,
//! `{"authorizedUsers":[..],"threshold":n}` or `null`, written compactly as `@nktkas/hyperliquid`
//! 0.33.3 writes it, the addresses lowercase and sorted as the venue's Python SDK sorts them. Once
//! converted, every action of the account must be sent through multi-sig, a change of signers or a
//! conversion back included; [`MultiSig`](crate::utils::sign::MultiSig) wraps L1 actions only.

use crate::account::{ActionKind, Signers};
use crate::utils::sign::{Field, NonceKey, UserAction, UserSigned};
use crate::utils::{Address, Json};

/// The most signers a multi-sig account may have.
pub const MOST_SIGNERS: usize = 10;

/// A set of signers the venue would not take.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BadSigners {
    /// Fewer than one signer, or more than [`MOST_SIGNERS`], once repeats are counted once.
    #[error("a multi-sig account has 1 to {MOST_SIGNERS} signers, not {0}")]
    Count(usize),
    /// A threshold of zero, or more than there are signers.
    #[error("a threshold is 1 to the number of signers ({signers}), not {threshold}")]
    Threshold {
        /// The threshold asked for.
        threshold: u8,
        /// How many distinct signers there are.
        signers: usize,
    },
}

/// `convertToMultiSigUser`, signed by the account's own key while it is a normal account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertToMultiSigUser {
    signers: String,
    nonce: u64,
}

impl ConvertToMultiSigUser {
    /// User-signed: only the account's own key.
    pub const KEY: Signers = ActionKind::ConvertToMultiSigUser.signers();

    /// Any `threshold` of `authorized` may act for the account. Each signer must already be an
    /// account on the venue; a repeated address counts once.
    pub fn new(authorized: &[Address], threshold: u8, nonce: u64) -> Result<ConvertToMultiSigUser, BadSigners> {
        let mut users: Vec<String> = authorized.iter().map(|user| user.to_hex()).collect();
        users.sort();
        users.dedup();
        if users.is_empty() || users.len() > MOST_SIGNERS {
            return Err(BadSigners::Count(users.len()));
        }
        if threshold == 0 || usize::from(threshold) > users.len() {
            return Err(BadSigners::Threshold { threshold, signers: users.len() });
        }
        let signers = Json::Object(vec![
            ("authorizedUsers".to_owned(), Json::Array(users.into_iter().map(Json::String).collect())),
            ("threshold".to_owned(), Json::Number(threshold.to_string())),
        ]);
        Ok(ConvertToMultiSigUser { signers: signers.compact(), nonce })
    }

    /// Back to a normal account: `signers` is `null`, as `@nktkas/hyperliquid` 0.33.3 writes it.
    #[must_use]
    pub fn single(nonce: u64) -> ConvertToMultiSigUser {
        ConvertToMultiSigUser { signers: "null".to_owned(), nonce }
    }

    /// `signers` as posted and signed.
    #[must_use]
    pub fn signers(&self) -> &str {
        &self.signers
    }
}

impl UserAction for ConvertToMultiSigUser {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new(
            "convertToMultiSigUser",
            "HyperliquidTransaction:ConvertToMultiSigUser",
            NonceKey::Nonce,
            self.nonce,
        )
        .field("signers", Field::String(&self.signers))
    }
}

#[cfg(test)]
#[path = "tests/multi_sig.rs"]
mod tests;
