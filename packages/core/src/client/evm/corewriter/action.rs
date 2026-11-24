//! [`Action`], every action in the docs' table with its fields in their units,
//! and its encoding: the fields as ABI values, the payload, the `sendRawAction`
//! calldata, the call and the transaction that sends it.

use crate::client::Network;
use crate::client::evm::abi::Token;
use crate::client::evm::corewriter::options::{Abstraction, BorrowLendOp, FinalizeVariant, OutcomeOp, Tif};
use crate::client::evm::corewriter::table::{
    CORE_WRITER, ENCODING_VERSION, ORDER_DECIMALS, SEND_RAW_ACTION, SPECS, Spec,
};
use crate::client::evm::tx::{Call, Eip1559, Fees};
use crate::client::evm::u256::{B256, U256};
use crate::utils::{Address, BadUnits, Dec, HYPE_DECIMALS, USDC_DECIMALS, exact_units};

/// Every action in the docs' table, its fields in their units.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    /// Action 1. `cloid` 0 is none.
    LimitOrder {
        /// The asset id, counted as [`Asset`](crate::utils::Asset) counts them.
        asset: u32,
        /// Whether the order buys.
        is_buy: bool,
        /// `limitPx`: the limit price times 1e8, whatever the market.
        limit_px: u64,
        /// The size times 1e8, whatever the market.
        sz: u64,
        /// `reduceOnly`: whether the order may only shrink a position.
        reduce_only: bool,
        /// `encodedTif`: the time in force.
        tif: Tif,
        /// The client order id; 0 is none.
        cloid: u128,
    },
    /// Action 2.
    VaultTransfer {
        /// The vault's address.
        vault: Address,
        /// `isDeposit`: true deposits into the vault, false withdraws from it.
        is_deposit: bool,
        /// The amount in USDC at 1e6.
        usd: u64,
    },
    /// Action 3.
    TokenDelegate {
        /// The validator's address.
        validator: Address,
        /// The amount in HYPE at 1e8.
        wei: u64,
        /// `isUndelegate`: true takes the stake back from the validator.
        is_undelegate: bool,
    },
    /// Action 4.
    StakingDeposit {
        /// The amount in HYPE at 1e8, moved from spot into staking.
        wei: u64,
    },
    /// Action 5.
    StakingWithdraw {
        /// The amount in HYPE at 1e8, moved from staking back to spot.
        wei: u64,
    },
    /// Action 6.
    SpotSend {
        /// The receiving address.
        destination: Address,
        /// The spot token index.
        token: u64,
        /// The amount in the token's `weiDecimals`.
        wei: u64,
    },
    /// Action 7.
    UsdClassTransfer {
        /// The amount in USDC at 1e6.
        ntl: u64,
        /// `toPerp`: true moves spot USDC to perps, false moves perp USDC to spot.
        to_perp: bool,
    },
    /// Action 8. `create_nonce` is read only by [`FinalizeVariant::Create`].
    FinalizeEvmContract {
        /// The spot token index being linked to its ERC-20.
        token: u64,
        /// `encodedFinalizeEvmContractVariant`: how the deployer is proven.
        variant: FinalizeVariant,
        /// `createNonce`: the nonce the contract was deployed at.
        create_nonce: u64,
    },
    /// Action 9. An empty name makes the wallet the main, unnamed agent.
    AddApiWallet {
        /// The agent wallet's address.
        wallet: Address,
        /// The agent's name; empty for the main, unnamed agent.
        name: String,
    },
    /// Action 10.
    CancelByOid {
        /// The asset id of the order's market.
        asset: u32,
        /// The order id HyperCore gave the order.
        oid: u64,
    },
    /// Action 11.
    CancelByCloid {
        /// The asset id of the order's market.
        asset: u32,
        /// The client order id the order was placed with.
        cloid: u128,
    },
    /// Action 12.
    ApproveBuilderFee {
        /// `maxFeeRate`: the most the builder may charge, in tenths of a basis point (10 is 0.01%).
        max_fee_rate: u64,
        /// The builder's address.
        builder: Address,
    },
    /// Action 13. A non-zero `sub_account` sends from that subaccount; a dex of
    /// [`SPOT_DEX`](crate::client::evm::corewriter::SPOT_DEX) is spot.
    SendAsset {
        /// The receiving address.
        destination: Address,
        /// `subAccount`: the subaccount to send from, or the zero address for the sender itself.
        sub_account: Address,
        /// `source_dex`: the dex the asset leaves, or `SPOT_DEX` for spot.
        source_dex: u32,
        /// `destination_dex`: the dex the asset arrives in, or `SPOT_DEX` for spot.
        destination_dex: u32,
        /// The token index.
        token: u64,
        /// The amount in the token's `weiDecimals`.
        wei: u64,
    },
    /// Action 15. `wei` 0 applies the operation to the most there is.
    BorrowLend {
        /// `encodedOperation`: supply, withdraw, borrow or repay.
        operation: BorrowLendOp,
        /// The token index.
        token: u64,
        /// The amount in the token's `weiDecimals`; 0 is the most there is.
        wei: u64,
    },
    /// Action 16.
    SetAbstraction {
        /// The master account or subaccount whose mode is set.
        user: Address,
        /// The account mode to set.
        abstraction: Abstraction,
    },
    /// Action 17. `wei` must be a multiple of the size; 0 merges the most there is.
    Outcome {
        /// `encodedOperation`: split, merge, merge question or negate.
        operation: OutcomeOp,
        /// The HIP-4 question id; must be 0 for split and merge.
        question: u32,
        /// The HIP-4 outcome id; must be 0 for merge question.
        outcome: u32,
        /// The amount, in the outcome token's `weiDecimals`; 0 merges the most there is.
        wei: u64,
    },
}

/// One field's encoding: a word, or text behind an offset.
enum Field {
    Word([u8; 32]),
    Text(Vec<u8>),
}

fn word(value: impl Into<U256>) -> Field {
    Field::Word(value.into().to_be_bytes())
}

fn address(address: Address) -> Field {
    Field::Word(B256::from_address(address).0)
}

fn flag(value: bool) -> Field {
    word(u8::from(value))
}

/// `abi.encode` of the fields: heads, then the tail of the one string.
fn encode(fields: &[Field]) -> Vec<u8> {
    let heads = fields.len() * 32;
    let mut head = Vec::with_capacity(heads);
    let mut tail = Vec::new();
    for field in fields {
        match field {
            Field::Word(word) => head.extend_from_slice(word),
            Field::Text(bytes) => {
                head.extend_from_slice(&U256::from_u64((heads + tail.len()) as u64).to_be_bytes());
                tail.extend_from_slice(&U256::from_u64(bytes.len() as u64).to_be_bytes());
                tail.extend_from_slice(bytes);
                tail.resize(tail.len().div_ceil(32) * 32, 0);
            }
        }
    }
    head.extend(tail);
    head
}

impl Action {
    /// The action id that goes in the payload header: 1 to 17, never 14.
    #[must_use]
    pub const fn id(&self) -> u32 {
        match self {
            Action::LimitOrder { .. } => 1,
            Action::VaultTransfer { .. } => 2,
            Action::TokenDelegate { .. } => 3,
            Action::StakingDeposit { .. } => 4,
            Action::StakingWithdraw { .. } => 5,
            Action::SpotSend { .. } => 6,
            Action::UsdClassTransfer { .. } => 7,
            Action::FinalizeEvmContract { .. } => 8,
            Action::AddApiWallet { .. } => 9,
            Action::CancelByOid { .. } => 10,
            Action::CancelByCloid { .. } => 11,
            Action::ApproveBuilderFee { .. } => 12,
            Action::SendAsset { .. } => 13,
            Action::BorrowLend { .. } => 15,
            Action::SetAbstraction { .. } => 16,
            Action::Outcome { .. } => 17,
        }
    }

    /// The docs' row for this action.
    #[must_use]
    pub fn spec(&self) -> &'static Spec {
        let id = self.id();
        SPECS.iter().find(|spec| spec.id == id).unwrap_or(&SPECS[0])
    }

    fn fields(&self) -> Vec<Field> {
        match self {
            Action::LimitOrder { asset, is_buy, limit_px, sz, reduce_only, tif, cloid } => vec![
                word(*asset),
                flag(*is_buy),
                word(*limit_px),
                word(*sz),
                flag(*reduce_only),
                word(tif.code()),
                word(*cloid),
            ],
            Action::VaultTransfer { vault, is_deposit, usd } => vec![address(*vault), flag(*is_deposit), word(*usd)],
            Action::TokenDelegate { validator, wei, is_undelegate } => {
                vec![address(*validator), word(*wei), flag(*is_undelegate)]
            }
            Action::StakingDeposit { wei } | Action::StakingWithdraw { wei } => vec![word(*wei)],
            Action::SpotSend { destination, token, wei } => vec![address(*destination), word(*token), word(*wei)],
            Action::UsdClassTransfer { ntl, to_perp } => vec![word(*ntl), flag(*to_perp)],
            Action::FinalizeEvmContract { token, variant, create_nonce } => {
                vec![word(*token), word(variant.code()), word(*create_nonce)]
            }
            Action::AddApiWallet { wallet, name } => vec![address(*wallet), Field::Text(name.as_bytes().to_vec())],
            Action::CancelByOid { asset, oid } => vec![word(*asset), word(*oid)],
            Action::CancelByCloid { asset, cloid } => vec![word(*asset), word(*cloid)],
            Action::ApproveBuilderFee { max_fee_rate, builder } => vec![word(*max_fee_rate), address(*builder)],
            Action::SendAsset { destination, sub_account, source_dex, destination_dex, token, wei } => vec![
                address(*destination),
                address(*sub_account),
                word(*source_dex),
                word(*destination_dex),
                word(*token),
                word(*wei),
            ],
            Action::BorrowLend { operation, token, wei } => vec![word(operation.code()), word(*token), word(*wei)],
            Action::SetAbstraction { user, abstraction } => vec![address(*user), word(abstraction.code())],
            Action::Outcome { operation, question, outcome, wei } => {
                vec![word(operation.code()), word(*question), word(*outcome), word(*wei)]
            }
        }
    }

    /// The fields as ABI values, in the docs' order and of the docs' types: what
    /// [`Action::from_tokens`] reads back, and what `abi::encode` over
    /// [`Spec::types`] writes as the payload after its header.
    #[must_use]
    pub fn tokens(&self) -> Vec<Token> {
        match self {
            Action::LimitOrder { asset, is_buy, limit_px, sz, reduce_only, tif, cloid } => vec![
                Token::uint(*asset),
                Token::Bool(*is_buy),
                Token::uint(*limit_px),
                Token::uint(*sz),
                Token::Bool(*reduce_only),
                Token::uint(tif.code()),
                Token::uint(*cloid),
            ],
            Action::VaultTransfer { vault, is_deposit, usd } => {
                vec![Token::Address(*vault), Token::Bool(*is_deposit), Token::uint(*usd)]
            }
            Action::TokenDelegate { validator, wei, is_undelegate } => {
                vec![Token::Address(*validator), Token::uint(*wei), Token::Bool(*is_undelegate)]
            }
            Action::StakingDeposit { wei } | Action::StakingWithdraw { wei } => vec![Token::uint(*wei)],
            Action::SpotSend { destination, token, wei } => {
                vec![Token::Address(*destination), Token::uint(*token), Token::uint(*wei)]
            }
            Action::UsdClassTransfer { ntl, to_perp } => vec![Token::uint(*ntl), Token::Bool(*to_perp)],
            Action::FinalizeEvmContract { token, variant, create_nonce } => {
                vec![Token::uint(*token), Token::uint(variant.code()), Token::uint(*create_nonce)]
            }
            Action::AddApiWallet { wallet, name } => vec![Token::Address(*wallet), Token::String(name.clone())],
            Action::CancelByOid { asset, oid } => vec![Token::uint(*asset), Token::uint(*oid)],
            Action::CancelByCloid { asset, cloid } => vec![Token::uint(*asset), Token::uint(*cloid)],
            Action::ApproveBuilderFee { max_fee_rate, builder } => {
                vec![Token::uint(*max_fee_rate), Token::Address(*builder)]
            }
            Action::SendAsset { destination, sub_account, source_dex, destination_dex, token, wei } => vec![
                Token::Address(*destination),
                Token::Address(*sub_account),
                Token::uint(*source_dex),
                Token::uint(*destination_dex),
                Token::uint(*token),
                Token::uint(*wei),
            ],
            Action::BorrowLend { operation, token, wei } => {
                vec![Token::uint(operation.code()), Token::uint(*token), Token::uint(*wei)]
            }
            Action::SetAbstraction { user, abstraction } => {
                vec![Token::Address(*user), Token::uint(abstraction.code())]
            }
            Action::Outcome { operation, question, outcome, wei } => {
                vec![Token::uint(operation.code()), Token::uint(*question), Token::uint(*outcome), Token::uint(*wei)]
            }
        }
    }

    /// The `data` of `sendRawAction`: the version, the id in three bytes, then
    /// the fields.
    #[must_use]
    pub fn payload(&self) -> Vec<u8> {
        let mut out = vec![ENCODING_VERSION];
        out.extend_from_slice(&self.id().to_be_bytes()[1..]);
        out.extend(encode(&self.fields()));
        out
    }

    /// The whole calldata of the `sendRawAction(bytes)` call.
    #[must_use]
    pub fn calldata(&self) -> Vec<u8> {
        let mut out = SEND_RAW_ACTION.to_vec();
        out.extend(encode(&[Field::Text(self.payload())]));
        out
    }

    /// The call to CoreWriter, with no value: value voids the action.
    #[must_use]
    pub fn call(&self) -> Call {
        Call { to: Some(CORE_WRITER), value: U256::ZERO, data: self.calldata() }
    }

    /// The HyperEVM transaction that sends it on `network`, from the account
    /// it acts for.
    #[must_use]
    pub fn transaction(&self, network: Network, nonce: u64, gas: u64, fees: Fees) -> Eip1559 {
        self.call().transaction(network.evm_chain_id(), nonce, gas, fees)
    }

    /// A limit order, its price and size as a person reads them, scaled to
    /// 1e8. More than eight places is refused, never rounded.
    pub fn limit_order(
        asset: u32,
        is_buy: bool,
        price: Dec,
        size: Dec,
        reduce_only: bool,
        tif: Tif,
        cloid: u128,
    ) -> Result<Action, BadUnits> {
        Ok(Action::LimitOrder {
            asset,
            is_buy,
            limit_px: exact_units(price, ORDER_DECIMALS)?,
            sz: exact_units(size, ORDER_DECIMALS)?,
            reduce_only,
            tif,
            cloid,
        })
    }

    /// USDC into or out of a vault, scaled to 1e6.
    pub fn vault_transfer(vault: Address, is_deposit: bool, usd: Dec) -> Result<Action, BadUnits> {
        Ok(Action::VaultTransfer { vault, is_deposit, usd: exact_units(usd, USDC_DECIMALS)? })
    }

    /// USDC between spot and perps, scaled to 1e6.
    pub fn usd_class_transfer(ntl: Dec, to_perp: bool) -> Result<Action, BadUnits> {
        Ok(Action::UsdClassTransfer { ntl: exact_units(ntl, USDC_DECIMALS)?, to_perp })
    }

    /// HYPE into staking, scaled to 1e8.
    pub fn staking_deposit(hype: Dec) -> Result<Action, BadUnits> {
        Ok(Action::StakingDeposit { wei: exact_units(hype, HYPE_DECIMALS)? })
    }

    /// HYPE out of staking, scaled to 1e8.
    pub fn staking_withdraw(hype: Dec) -> Result<Action, BadUnits> {
        Ok(Action::StakingWithdraw { wei: exact_units(hype, HYPE_DECIMALS)? })
    }

    /// Staked HYPE to or from a validator, scaled to 1e8.
    pub fn token_delegate(validator: Address, hype: Dec, is_undelegate: bool) -> Result<Action, BadUnits> {
        Ok(Action::TokenDelegate { validator, wei: exact_units(hype, HYPE_DECIMALS)?, is_undelegate })
    }
}

#[cfg(test)]
#[path = "tests/action.rs"]
mod tests;
