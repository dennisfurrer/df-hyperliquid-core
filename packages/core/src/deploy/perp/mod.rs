//! Deploying and running a HIP-3 perp dex: every `perpDeploy` variant the docs list, all L1 actions.
//! The deploy auction read is in [`dexes`](crate::deploy::dexes). The testnet-only HIP-3* `star`
//! operations are not here.

pub mod asset;
pub mod margin;
pub mod sub;

pub use asset::{DexSchema, MarginMode, NewAsset};
pub use margin::{MarginTable, MarginTier};
pub use sub::{PerpKind, SubDeployer};

use std::collections::{BTreeMap, btree_map::Entry};

use crate::deploy::annotation::Annotation;
use crate::deploy::error::BadDeploy;
use crate::deploy::name::DexName;
use crate::utils::sign::{L1Action, Map, Value};
use crate::utils::{Address, Dec};

/// Values by coin (`dex:NAME`), kept in the order the venue hashes them:
/// bytewise by coin, so `xyz:BTC` comes before `xyz:abc`.
pub type ByCoin<T> = BTreeMap<String, T>;

/// A deployer's fee setting for one asset: the scale applied to user fees
/// and whether growth mode is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeployerFee {
    /// From 0 to 3, or from 0 up to (not including) 10 in growth mode.
    pub scale: Dec,
    /// Growth mode (`growthMode`): when on, the scale may go up to (not including) 10.
    pub growth_mode: bool,
}

/// `{"type":"perpDeploy",<variant>:..}`. The constructors check what the
/// docs rule out and take each per-coin list in any order, refusing a coin
/// given twice; the lists go out sorted by coin, as the venue requires.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PerpDeploy {
    /// `registerAsset2`: an asset with a margin mode. With a schema it also
    /// creates the dex. `max_gas` is in HYPE wei, 8 decimals; `None` (sent as
    /// null) pays the current auction price, and 0 uses a reserve
    /// deployment whether or not the auction has ended.
    RegisterAsset2 {
        /// The most to pay (`maxGas`), in HYPE wei, 8 decimals (1 HYPE is `100_000_000`); `None`,
        /// sent as null, pays the current auction price. The gas paid is not refunded.
        max_gas: Option<u64>,
        /// The asset to list (`assetRequest`).
        asset: NewAsset,
        /// How the asset may be margined (`marginMode`).
        margin_mode: MarginMode,
        /// The dex the asset is listed on.
        dex: DexName,
        /// The new dex's schema when this asset creates the dex; `None`, sent as null, for an existing dex.
        schema: Option<DexSchema>,
    },
    /// `registerAsset`: as [`RegisterAsset2`](PerpDeploy::RegisterAsset2),
    /// with `onlyIsolated` for the margin mode.
    RegisterAsset {
        /// The most to pay (`maxGas`), in HYPE wei, 8 decimals (1 HYPE is `100_000_000`); `None`,
        /// sent as null, pays the current auction price. The gas paid is not refunded.
        max_gas: Option<u64>,
        /// The asset to list (`assetRequest`).
        asset: NewAsset,
        /// Whether the asset is isolated margin only (`onlyIsolated`).
        only_isolated: bool,
        /// The dex the asset is listed on.
        dex: DexName,
        /// The new dex's schema when this asset creates the dex; `None`, sent as null, for an existing dex.
        schema: Option<DexSchema>,
    },
    /// `setOracle`: oracle prices, 0 to 2 lists of mark prices, and external
    /// perp prices for every asset. At least 2.5 seconds apart.
    SetOracle {
        /// The dex the prices are for.
        dex: DexName,
        /// Oracle prices by coin (`oraclePxs`).
        oracle_pxs: ByCoin<Dec>,
        /// 0 to 2 lists of mark prices by coin (`markPxs`).
        mark_pxs: Vec<ByCoin<Dec>>,
        /// External perp prices by coin (`externalPerpPxs`).
        external_perp_pxs: ByCoin<Dec>,
    },
    /// `setFundingMultipliers`, each from 0 to 10.
    SetFundingMultipliers(ByCoin<Dec>),
    /// `setFundingInterestRates`: 8 hour rates, each from -0.01 to 0.01.
    SetFundingInterestRates(ByCoin<Dec>),
    /// `setFundingClamps`: 8 hour clamps, each from 0 to 0.01.
    SetFundingClamps(ByCoin<Dec>),
    /// `haltTrading`: halts (`true`) or resumes (`false`) one asset.
    HaltTrading {
        /// The coin, `dex:NAME`.
        coin: String,
        /// `true` halts the asset, `false` resumes it (`isHalted`).
        is_halted: bool,
    },
    /// `insertMarginTable`.
    InsertMarginTable {
        /// The dex the table is for.
        dex: DexName,
        /// The table to insert (`marginTable`).
        margin_table: MarginTable,
    },
    /// `setMarginTableIds`, none of them 0.
    SetMarginTableIds(ByCoin<u32>),
    /// `setFeeRecipient`.
    SetFeeRecipient {
        /// The dex whose fees are paid out.
        dex: DexName,
        /// The address the deployer's fee share goes to (`feeRecipient`).
        fee_recipient: Address,
    },
    /// `setOpenInterestCaps`: notional caps; `None` (sent as null) removes
    /// an asset's custom cap. A cap is in millionths of the dex's collateral token whatever its
    /// `weiDecimals`: `1_000_000` is one token of notional. `perpDexLimits` shows it in whole
    /// tokens. Confirmed against mainnet caps on USDC, USDE, USDH and USDT0 dexes.
    SetOpenInterestCaps(ByCoin<Option<u64>>),
    /// `setSubDeployers`.
    SetSubDeployers {
        /// The dex the grants are for.
        dex: DexName,
        /// The grants (`subDeployers`), at least one.
        sub_deployers: Vec<SubDeployer>,
    },
    /// `setMarginModes`.
    SetMarginModes(ByCoin<MarginMode>),
    /// `setDeployerFees`. On mainnet, one change per 30 days.
    SetDeployerFees(ByCoin<DeployerFee>),
    /// `setPerpAnnotation`.
    SetPerpAnnotation {
        /// The coin, `dex:NAME`.
        coin: String,
        /// The annotation, sent flat next to the coin.
        annotation: Annotation,
    },
    /// `disableDex`.
    DisableDex(DexName),
}

/// `pairs` by coin, a coin given twice refused.
fn by_coin<K: Into<String>, T>(pairs: impl IntoIterator<Item = (K, T)>) -> Result<ByCoin<T>, BadDeploy> {
    let mut coins = ByCoin::new();
    for (coin, value) in pairs {
        match coins.entry(coin.into()) {
            Entry::Occupied(taken) => return Err(BadDeploy::Twice(taken.key().clone())),
            Entry::Vacant(slot) => {
                slot.insert(value);
            }
        }
    }
    Ok(coins)
}

/// `value` if it is from `least` to `most`.
fn within(
    what: &'static str,
    value: Dec,
    (least, least_text): (Dec, &'static str),
    (most, most_text): (Dec, &'static str),
) -> Result<Dec, BadDeploy> {
    if value < least || value > most {
        return Err(BadDeploy::OutOfRange { what, least: least_text, most: most_text, value: value.to_wire() });
    }
    Ok(value)
}

/// Each value of `coins` checked by `check`.
fn checked<T: Copy>(coins: &ByCoin<T>, check: impl Fn(T) -> Result<(), BadDeploy>) -> Result<(), BadDeploy> {
    coins.values().try_for_each(|value| check(*value))
}

fn not_negative(what: &'static str, value: Dec) -> Result<(), BadDeploy> {
    if value.is_negative() {
        return Err(BadDeploy::OutOfRange { what, least: "0", most: "any", value: value.to_wire() });
    }
    Ok(())
}

/// `[[coin, value], ..]` in coin order.
fn tuples<T>(coins: &ByCoin<T>, value: impl Fn(&T) -> Value) -> Value {
    Value::array(coins.iter().map(|(coin, one)| Value::Array(vec![coin.clone().into(), value(one)])))
}

fn decimals(coins: &ByCoin<Dec>) -> Value {
    tuples(coins, |value| value.to_wire().into())
}

const ZERO: (Dec, &str) = (Dec::ZERO, "0");
const TEN: (Dec, &str) = (Dec::lit::<0>(10), "10");
const RATE: Dec = Dec::lit::<2>(1);

impl PerpDeploy {
    /// `registerAsset2`, refusing a negative oracle price.
    pub fn register_asset2(
        dex: DexName,
        max_gas: Option<u64>,
        asset: NewAsset,
        margin_mode: MarginMode,
        schema: Option<DexSchema>,
    ) -> Result<Self, BadDeploy> {
        not_negative("oraclePx", asset.oracle_px)?;
        Ok(PerpDeploy::RegisterAsset2 { max_gas, asset, margin_mode, dex, schema })
    }

    /// `registerAsset`, refusing a negative oracle price.
    pub fn register_asset(
        dex: DexName,
        max_gas: Option<u64>,
        asset: NewAsset,
        only_isolated: bool,
        schema: Option<DexSchema>,
    ) -> Result<Self, BadDeploy> {
        not_negative("oraclePx", asset.oracle_px)?;
        Ok(PerpDeploy::RegisterAsset { max_gas, asset, only_isolated, dex, schema })
    }

    /// `setOracle`, refusing more than 2 mark price lists or a negative price.
    pub fn set_oracle<K: Into<String>>(
        dex: DexName,
        oracle_pxs: impl IntoIterator<Item = (K, Dec)>,
        mark_pxs: Vec<Vec<(K, Dec)>>,
        external_perp_pxs: impl IntoIterator<Item = (K, Dec)>,
    ) -> Result<Self, BadDeploy> {
        if mark_pxs.len() > 2 {
            return Err(BadDeploy::MarkPxs(mark_pxs.len()));
        }
        let oracle_pxs = by_coin(oracle_pxs)?;
        let mark_pxs = mark_pxs.into_iter().map(by_coin).collect::<Result<Vec<_>, _>>()?;
        let external_perp_pxs = by_coin(external_perp_pxs)?;
        checked(&oracle_pxs, |px| not_negative("an oracle price", px))?;
        mark_pxs.iter().try_for_each(|marks| checked(marks, |px| not_negative("a mark price", px)))?;
        checked(&external_perp_pxs, |px| not_negative("an external perp price", px))?;
        Ok(PerpDeploy::SetOracle { dex, oracle_pxs, mark_pxs, external_perp_pxs })
    }

    /// `setFundingMultipliers`, each from 0 to 10.
    pub fn set_funding_multipliers<K: Into<String>>(
        multipliers: impl IntoIterator<Item = (K, Dec)>,
    ) -> Result<Self, BadDeploy> {
        let multipliers = by_coin(multipliers)?;
        checked(&multipliers, |value| within("a funding multiplier", value, ZERO, TEN).map(drop))?;
        Ok(PerpDeploy::SetFundingMultipliers(multipliers))
    }

    /// `setFundingInterestRates`, each from -0.01 to 0.01.
    pub fn set_funding_interest_rates<K: Into<String>>(
        rates: impl IntoIterator<Item = (K, Dec)>,
    ) -> Result<Self, BadDeploy> {
        let rates = by_coin(rates)?;
        checked(&rates, |value| {
            within("a funding interest rate", value, (RATE.negate(), "-0.01"), (RATE, "0.01")).map(drop)
        })?;
        Ok(PerpDeploy::SetFundingInterestRates(rates))
    }

    /// `setFundingClamps`, each from 0 to 0.01.
    pub fn set_funding_clamps<K: Into<String>>(clamps: impl IntoIterator<Item = (K, Dec)>) -> Result<Self, BadDeploy> {
        let clamps = by_coin(clamps)?;
        checked(&clamps, |value| within("a funding clamp", value, ZERO, (RATE, "0.01")).map(drop))?;
        Ok(PerpDeploy::SetFundingClamps(clamps))
    }

    /// `setMarginTableIds`, refusing id 0.
    pub fn set_margin_table_ids<K: Into<String>>(ids: impl IntoIterator<Item = (K, u32)>) -> Result<Self, BadDeploy> {
        let ids = by_coin(ids)?;
        if let Some((coin, _)) = ids.iter().find(|(_, id)| **id == 0) {
            return Err(BadDeploy::MarginTableZero(coin.clone()));
        }
        Ok(PerpDeploy::SetMarginTableIds(ids))
    }

    /// `setOpenInterestCaps`; `None` removes an asset's custom cap. Caps are in millionths of the
    /// collateral token, as [`PerpDeploy::SetOpenInterestCaps`] says.
    pub fn set_open_interest_caps<K: Into<String>>(
        caps: impl IntoIterator<Item = (K, Option<u64>)>,
    ) -> Result<Self, BadDeploy> {
        Ok(PerpDeploy::SetOpenInterestCaps(by_coin(caps)?))
    }

    /// `setSubDeployers`, refusing an empty list.
    pub fn set_sub_deployers(dex: DexName, sub_deployers: Vec<SubDeployer>) -> Result<Self, BadDeploy> {
        if sub_deployers.is_empty() {
            return Err(BadDeploy::NoGrants);
        }
        Ok(PerpDeploy::SetSubDeployers { dex, sub_deployers })
    }

    /// `setMarginModes`.
    pub fn set_margin_modes<K: Into<String>>(
        modes: impl IntoIterator<Item = (K, MarginMode)>,
    ) -> Result<Self, BadDeploy> {
        Ok(PerpDeploy::SetMarginModes(by_coin(modes)?))
    }

    /// `setDeployerFees`: each scale from 0 to 3, or from 0 up to (not
    /// including) 10 in growth mode.
    pub fn set_deployer_fees<K: Into<String>>(
        fees: impl IntoIterator<Item = (K, DeployerFee)>,
    ) -> Result<Self, BadDeploy> {
        let fees = by_coin(fees)?;
        checked(&fees, |fee| {
            if fee.growth_mode {
                if fee.scale >= TEN.0 {
                    return Err(BadDeploy::OutOfRange {
                        what: "a growth mode fee scale",
                        least: "0",
                        most: "just under 10",
                        value: fee.scale.to_wire(),
                    });
                }
                not_negative("a growth mode fee scale", fee.scale)
            } else {
                within("a fee scale", fee.scale, ZERO, (Dec::lit::<0>(3), "3")).map(drop)
            }
        })?;
        Ok(PerpDeploy::SetDeployerFees(fees))
    }

    /// The variant, by the key it travels under.
    #[must_use]
    pub const fn kind(&self) -> PerpKind {
        match self {
            PerpDeploy::RegisterAsset2 { .. } => PerpKind::RegisterAsset2,
            PerpDeploy::RegisterAsset { .. } => PerpKind::RegisterAsset,
            PerpDeploy::SetOracle { .. } => PerpKind::SetOracle,
            PerpDeploy::SetFundingMultipliers(_) => PerpKind::SetFundingMultipliers,
            PerpDeploy::SetFundingInterestRates(_) => PerpKind::SetFundingInterestRates,
            PerpDeploy::SetFundingClamps(_) => PerpKind::SetFundingClamps,
            PerpDeploy::HaltTrading { .. } => PerpKind::HaltTrading,
            PerpDeploy::InsertMarginTable { .. } => PerpKind::InsertMarginTable,
            PerpDeploy::SetMarginTableIds(_) => PerpKind::SetMarginTableIds,
            PerpDeploy::SetFeeRecipient { .. } => PerpKind::SetFeeRecipient,
            PerpDeploy::SetOpenInterestCaps(_) => PerpKind::SetOpenInterestCaps,
            PerpDeploy::SetSubDeployers { .. } => PerpKind::SetSubDeployers,
            PerpDeploy::SetMarginModes(_) => PerpKind::SetMarginModes,
            PerpDeploy::SetDeployerFees(_) => PerpKind::SetDeployerFees,
            PerpDeploy::SetPerpAnnotation { .. } => PerpKind::SetPerpAnnotation,
            PerpDeploy::DisableDex(_) => PerpKind::DisableDex,
        }
    }

    /// The value under the variant's key, keys in the order the docs and
    /// the venue's clients write them.
    #[must_use]
    pub fn body(&self) -> Value {
        let gas = |max_gas: &Option<u64>| max_gas.map_or(Value::Nil, Value::from);
        let schema = |schema: &Option<DexSchema>| schema.as_ref().map_or(Value::Nil, |one| one.wire().into());
        match self {
            PerpDeploy::RegisterAsset2 { max_gas, asset, margin_mode, dex, schema: new } => Map::new()
                .put("maxGas", gas(max_gas))
                .put("assetRequest", asset.wire().put("marginMode", margin_mode.wire()))
                .put("dex", dex.as_str().to_owned())
                .put("schema", schema(new))
                .into(),
            PerpDeploy::RegisterAsset { max_gas, asset, only_isolated, dex, schema: new } => Map::new()
                .put("maxGas", gas(max_gas))
                .put("assetRequest", asset.wire().put("onlyIsolated", *only_isolated))
                .put("dex", dex.as_str().to_owned())
                .put("schema", schema(new))
                .into(),
            PerpDeploy::SetOracle { dex, oracle_pxs, mark_pxs, external_perp_pxs } => Map::new()
                .put("dex", dex.as_str().to_owned())
                .put("oraclePxs", decimals(oracle_pxs))
                .put("markPxs", Value::array(mark_pxs.iter().map(decimals)))
                .put("externalPerpPxs", decimals(external_perp_pxs))
                .into(),
            PerpDeploy::SetFundingMultipliers(values)
            | PerpDeploy::SetFundingInterestRates(values)
            | PerpDeploy::SetFundingClamps(values) => decimals(values),
            PerpDeploy::HaltTrading { coin, is_halted } => {
                Map::new().put("coin", coin.clone()).put("isHalted", *is_halted).into()
            }
            PerpDeploy::InsertMarginTable { dex, margin_table } => {
                Map::new().put("dex", dex.as_str().to_owned()).put("marginTable", margin_table.wire()).into()
            }
            PerpDeploy::SetMarginTableIds(ids) => tuples(ids, |id| (*id).into()),
            PerpDeploy::SetFeeRecipient { dex, fee_recipient } => {
                Map::new().put("dex", dex.as_str().to_owned()).put("feeRecipient", fee_recipient.to_hex()).into()
            }
            PerpDeploy::SetOpenInterestCaps(caps) => tuples(caps, |cap| cap.map_or(Value::Nil, Value::from)),
            PerpDeploy::SetSubDeployers { dex, sub_deployers } => Map::new()
                .put("dex", dex.as_str().to_owned())
                .put("subDeployers", Value::array(sub_deployers.iter().map(SubDeployer::wire)))
                .into(),
            PerpDeploy::SetMarginModes(modes) => tuples(modes, |mode| mode.wire().into()),
            PerpDeploy::SetDeployerFees(fees) => tuples(fees, |fee| {
                Map::new().put("scale", fee.scale.to_wire()).put("growthMode", fee.growth_mode).into()
            }),
            PerpDeploy::SetPerpAnnotation { coin, annotation } => {
                annotation.wire_after(Map::new().put("coin", coin.clone())).into()
            }
            PerpDeploy::DisableDex(dex) => dex.as_str().to_owned().into(),
        }
    }
}

impl L1Action for PerpDeploy {
    fn action(&self) -> Map {
        Map::new().put("type", "perpDeploy").put(self.kind().wire(), self.body())
    }
}

#[cfg(test)]
#[path = "tests/perp.rs"]
mod tests;
