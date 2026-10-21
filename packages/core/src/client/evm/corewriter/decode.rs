//! An [`Action`] back out of a payload or out of its fields as ABI values: the
//! header checked, the fields read against the docs' types, and [`BadAction`]
//! when they are not one.

use crate::client::evm::abi::{self, AbiError, Token};
use crate::client::evm::corewriter::action::Action;
use crate::client::evm::corewriter::options::{Abstraction, BorrowLendOp, FinalizeVariant, OutcomeOp, Tif};
use crate::client::evm::corewriter::table::{ENCODING_VERSION, GAP, Spec, spec};
use crate::utils::Address;

/// Why bytes are not a CoreWriter action this reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadAction {
    #[error("not a RawAction log of CoreWriter's")]
    NotRawAction,
    #[error("{0} bytes is shorter than the four-byte header")]
    Short(usize),
    #[error("encoding version {0}; the docs give only 1")]
    Version(u8),
    #[error("action 14, the gap in the docs' table; it has no published meaning")]
    Gap,
    #[error("action {0} is not in the docs' table")]
    UnknownId(u32),
    #[error(transparent)]
    Abi(#[from] AbiError),
}

impl Action {
    /// An action back out of a payload: the header checked, the fields read
    /// against the docs' types.
    pub fn decode(payload: &[u8]) -> Result<Action, BadAction> {
        let Some((header, body)) = payload.split_first_chunk::<4>() else {
            return Err(BadAction::Short(payload.len()));
        };
        if header[0] != ENCODING_VERSION {
            return Err(BadAction::Version(header[0]));
        }
        let id = u32::from_be_bytes([0, header[1], header[2], header[3]]);
        let tokens = abi::decode(&abi::params(known(id)?.types)?, body)?;
        Action::from_tokens(id, &tokens)
    }

    /// An action from its fields as ABI values, in the docs' order and of the docs' types:
    /// what [`Action::tokens`] gives, or text read with [`Token::parse`]. A value of another
    /// type or outside its width is refused, as is an id the table does not have.
    pub fn from_tokens(id: u32, tokens: &[Token]) -> Result<Action, BadAction> {
        let wants = known(id)?.fields.len();
        if tokens.len() != wants {
            return Err(BadAction::Abi(AbiError::Count { wants, got: tokens.len() }));
        }
        let mut fields = Reader { items: tokens.iter() };
        let r = &mut fields;
        Ok(match id {
            1 => Action::LimitOrder {
                asset: r.u32()?,
                is_buy: r.bool()?,
                limit_px: r.u64()?,
                sz: r.u64()?,
                reduce_only: r.bool()?,
                tif: Tif::from_code(r.u8()?),
                cloid: r.u128()?,
            },
            2 => Action::VaultTransfer { vault: r.address()?, is_deposit: r.bool()?, usd: r.u64()? },
            3 => Action::TokenDelegate { validator: r.address()?, wei: r.u64()?, is_undelegate: r.bool()? },
            4 => Action::StakingDeposit { wei: r.u64()? },
            5 => Action::StakingWithdraw { wei: r.u64()? },
            6 => Action::SpotSend { destination: r.address()?, token: r.u64()?, wei: r.u64()? },
            7 => Action::UsdClassTransfer { ntl: r.u64()?, to_perp: r.bool()? },
            8 => Action::FinalizeEvmContract {
                token: r.u64()?,
                variant: FinalizeVariant::from_code(r.u8()?),
                create_nonce: r.u64()?,
            },
            9 => Action::AddApiWallet { wallet: r.address()?, name: r.string()? },
            10 => Action::CancelByOid { asset: r.u32()?, oid: r.u64()? },
            11 => Action::CancelByCloid { asset: r.u32()?, cloid: r.u128()? },
            12 => Action::ApproveBuilderFee { max_fee_rate: r.u64()?, builder: r.address()? },
            13 => Action::SendAsset {
                destination: r.address()?,
                sub_account: r.address()?,
                source_dex: r.u32()?,
                destination_dex: r.u32()?,
                token: r.u64()?,
                wei: r.u64()?,
            },
            15 => Action::BorrowLend { operation: BorrowLendOp::from_code(r.u8()?), token: r.u64()?, wei: r.u64()? },
            16 => Action::SetAbstraction { user: r.address()?, abstraction: Abstraction::from_code(r.u8()?) },
            _ => Action::Outcome {
                operation: OutcomeOp::from_code(r.u8()?),
                question: r.u32()?,
                outcome: r.u32()?,
                wei: r.u64()?,
            },
        })
    }
}

/// The table's row for `id`, or why there is none.
fn known(id: u32) -> Result<&'static Spec, BadAction> {
    match (spec(id), id) {
        (Some(spec), _) => Ok(spec),
        (None, GAP) => Err(BadAction::Gap),
        (None, _) => Err(BadAction::UnknownId(id)),
    }
}

/// The decoded fields of an action, read in order.
struct Reader<'a> {
    items: std::slice::Iter<'a, Token>,
}

impl<'a> Reader<'a> {
    fn take<T>(&mut self, read: impl FnOnce(&'a Token) -> Option<T>) -> Result<T, BadAction> {
        self.items.next().and_then(read).ok_or(BadAction::Abi(AbiError::Type("a field of another type".to_owned())))
    }

    fn u128(&mut self) -> Result<u128, BadAction> {
        self.take(Token::as_u128)
    }

    fn u64(&mut self) -> Result<u64, BadAction> {
        self.take(Token::as_u64)
    }

    fn u32(&mut self) -> Result<u32, BadAction> {
        self.take(Token::as_u32)
    }

    fn u8(&mut self) -> Result<u8, BadAction> {
        self.take(Token::as_u8)
    }

    fn bool(&mut self) -> Result<bool, BadAction> {
        self.take(Token::as_bool)
    }

    fn address(&mut self) -> Result<Address, BadAction> {
        self.take(Token::as_address)
    }

    fn string(&mut self) -> Result<String, BadAction> {
        self.take(|token| token.as_str().map(ToOwned::to_owned))
    }
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
