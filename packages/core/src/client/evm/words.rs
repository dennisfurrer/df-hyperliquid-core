//! CoreWriter actions in words: an asset id becomes its market, a token index
//! its name, and every amount is scaled by its unit ([`crate::client::evm::corewriter`]).

use std::collections::BTreeMap;

use crate::client::evm::corewriter::{
    Abstraction, Action, BorrowLendOp, DECIBPS_DECIMALS, FinalizeVariant, HYPE_DECIMALS, ORDER_DECIMALS, OutcomeOp,
    SPOT_DEX, Tif, USD_DECIMALS,
};
use crate::utils::{Address, Dec};

/// What a network calls its markets, tokens and dexes, to put names to ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    /// By the asset id an order names a market by.
    pub markets: BTreeMap<u32, Market>,
    /// By token index.
    pub tokens: BTreeMap<u64, TokenName>,
    /// The HIP-3 dexes in `perpDexs` order, the main dex left out.
    pub dexes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Market {
    pub name: String,
    pub spot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenName {
    pub name: String,
    pub wei_decimals: u32,
}

impl Names {
    /// The market an asset id names, or `asset <id>`.
    #[must_use]
    pub fn market(&self, asset: u32) -> String {
        self.markets.get(&asset).map_or_else(|| format!("asset {asset}"), |market| market.name.clone())
    }

    /// A token's name, or `token <index>`.
    #[must_use]
    pub fn token(&self, index: u64) -> String {
        self.tokens.get(&index).map_or_else(|| format!("token {index}"), |token| token.name.clone())
    }

    /// An amount of a token in its wei, scaled by its decimals when they are known.
    #[must_use]
    pub fn amount(&self, index: u64, wei: u64) -> String {
        match self.tokens.get(&index) {
            Some(token) => format!("{} {}", scaled(u128::from(wei), token.wei_decimals), token.name),
            None => format!("{wei} wei of token {index}"),
        }
    }

    /// A dex as `sendAsset` numbers it: spot, the main perp dex, or a HIP-3 one.
    #[must_use]
    pub fn dex(&self, dex: u32) -> String {
        match dex {
            SPOT_DEX => "spot".to_owned(),
            0 => "perp".to_owned(),
            at => usize::try_from(at - 1)
                .ok()
                .and_then(|at| self.dexes.get(at))
                .map_or_else(|| format!("dex {dex}"), |name| format!("{name} perp")),
        }
    }
}

/// `units` at `decimals` places, or the units as they are past what a [`Dec`] keeps.
fn scaled(units: u128, decimals: u32) -> String {
    i128::try_from(units)
        .ok()
        .and_then(|units| Dec::new(units, decimals))
        .map_or_else(|| units.to_string(), |dec| dec.to_string())
}

/// An address cut to its ends: `0x7d1c..fe05`.
#[must_use]
pub fn short(address: Address) -> String {
    let text = address.to_hex();
    format!("{}..{}", &text[..6], &text[text.len() - 4..])
}

fn hype(wei: u64) -> String {
    format!("{} HYPE", scaled(u128::from(wei), HYPE_DECIMALS))
}

fn usd(units: u64) -> String {
    format!("{} USDC", scaled(u128::from(units), USD_DECIMALS))
}

fn tif(tif: Tif) -> String {
    match tif {
        Tif::Alo => "Alo".to_owned(),
        Tif::Gtc => "Gtc".to_owned(),
        Tif::Ioc => "Ioc".to_owned(),
        Tif::Other(code) => format!("encodedTif {code}"),
    }
}

fn variant(variant: FinalizeVariant) -> String {
    match variant {
        FinalizeVariant::Create => "Create".to_owned(),
        FinalizeVariant::FirstStorageSlot => "FirstStorageSlot".to_owned(),
        FinalizeVariant::CustomStorageSlot => "CustomStorageSlot".to_owned(),
        FinalizeVariant::Other(code) => format!("encodedFinalizeEvmContractVariant {code}"),
    }
}

fn lending(operation: BorrowLendOp) -> String {
    match operation {
        BorrowLendOp::Supply => "Supply".to_owned(),
        BorrowLendOp::Withdraw => "Withdraw".to_owned(),
        BorrowLendOp::Borrow => "Borrow (undocumented)".to_owned(),
        BorrowLendOp::Repay => "Repay (undocumented)".to_owned(),
        BorrowLendOp::Other(code) => format!("encodedOperation {code}"),
    }
}

fn mode(abstraction: Abstraction) -> String {
    match abstraction {
        Abstraction::Disabled => "disabled".to_owned(),
        Abstraction::UnifiedAccount => "unifiedAccount".to_owned(),
        Abstraction::PortfolioMargin => "portfolioMargin".to_owned(),
        Abstraction::Other(code) => format!("abstraction {code}"),
    }
}

fn outcome(operation: OutcomeOp) -> String {
    match operation {
        OutcomeOp::SplitOutcome => "SplitOutcome".to_owned(),
        OutcomeOp::MergeOutcome => "MergeOutcome".to_owned(),
        OutcomeOp::MergeQuestion => "MergeQuestion".to_owned(),
        OutcomeOp::NegateOutcome => "NegateOutcome".to_owned(),
        OutcomeOp::Other(code) => format!("encodedOperation {code}"),
    }
}

impl Action {
    /// The action on one line, as a person would say it.
    #[must_use]
    pub fn describe(&self, names: &Names) -> String {
        match self {
            Action::LimitOrder { asset, is_buy, limit_px, sz, reduce_only, tif: time, .. } => {
                let side = if *is_buy { "buy" } else { "sell" };
                let mut said = format!(
                    "{side} {} {} at {}",
                    scaled(u128::from(*sz), ORDER_DECIMALS),
                    names.market(*asset),
                    scaled(u128::from(*limit_px), ORDER_DECIMALS)
                );
                if *reduce_only {
                    said.push_str("  reduce only");
                }
                format!("{said}  {}", tif(*time))
            }
            Action::VaultTransfer { vault, is_deposit: true, usd: amount } => {
                format!("deposit {} into {}", usd(*amount), short(*vault))
            }
            Action::VaultTransfer { vault, is_deposit: false, usd: amount } => {
                format!("withdraw {} from {}", usd(*amount), short(*vault))
            }
            Action::TokenDelegate { validator, wei, is_undelegate: false } => {
                format!("delegate {} to {}", hype(*wei), short(*validator))
            }
            Action::TokenDelegate { validator, wei, is_undelegate: true } => {
                format!("undelegate {} from {}", hype(*wei), short(*validator))
            }
            Action::StakingDeposit { wei } => format!("{} into staking", hype(*wei)),
            Action::StakingWithdraw { wei } => format!("{} out of staking", hype(*wei)),
            Action::SpotSend { destination, token, wei } => {
                format!("{} to {}", names.amount(*token, *wei), short(*destination))
            }
            Action::UsdClassTransfer { ntl, to_perp } => {
                format!("{}  {}", usd(*ntl), if *to_perp { "spot to perp" } else { "perp to spot" })
            }
            Action::FinalizeEvmContract { token, variant: how, create_nonce } => {
                format!("{}  {}  nonce {create_nonce}", names.token(*token), variant(*how))
            }
            Action::AddApiWallet { wallet, name } => match name.as_str() {
                "" => format!("{}  unnamed", short(*wallet)),
                name => format!("{}  named {name}", short(*wallet)),
            },
            Action::CancelByOid { asset, oid } => format!("{}  oid {oid}", names.market(*asset)),
            Action::CancelByCloid { asset, cloid } => format!("{}  cloid 0x{cloid:032x}", names.market(*asset)),
            Action::ApproveBuilderFee { max_fee_rate, builder } => {
                format!("up to {}% for {}", scaled(u128::from(*max_fee_rate), DECIBPS_DECIMALS), short(*builder))
            }
            Action::SendAsset { destination, sub_account, source_dex, destination_dex, token, wei } => {
                let mut said = format!(
                    "{} to {}  {} to {}",
                    names.amount(*token, *wei),
                    short(*destination),
                    names.dex(*source_dex),
                    names.dex(*destination_dex)
                );
                if !sub_account.is_zero() {
                    said.push_str(&format!("  from sub {}", short(*sub_account)));
                }
                said
            }
            Action::BorrowLend { operation, token, wei } => match wei {
                0 => format!("{} the most there is of {}", lending(*operation), names.token(*token)),
                _ => format!("{} {}", lending(*operation), names.amount(*token, *wei)),
            },
            Action::SetAbstraction { user, abstraction } => format!("{} for {}", mode(*abstraction), short(*user)),
            Action::Outcome { operation, question, outcome: id, wei } => {
                let amount = match wei {
                    0 => "the most there is".to_owned(),
                    _ => format!("{wei} wei"),
                };
                match operation {
                    OutcomeOp::MergeQuestion => format!("{}  question {question}  {amount}", outcome(*operation)),
                    OutcomeOp::NegateOutcome => {
                        format!("{}  question {question} outcome {id}  {amount}", outcome(*operation))
                    }
                    _ => format!("{}  outcome {id}  {amount}", outcome(*operation)),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/words.rs"]
mod tests;
