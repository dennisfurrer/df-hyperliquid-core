//! Sub-deployers: which `perpDeploy` variant another address may send for a dex ([`PerpKind`]), and
//! one address let in to or shut out of it ([`SubDeployer`]).

use crate::utils::Address;
use crate::utils::sign::Map;

/// A `perpDeploy` variant, by the key it travels under: what a
/// sub-deployer grant names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerpKind {
    RegisterAsset2,
    RegisterAsset,
    SetOracle,
    SetFundingMultipliers,
    SetFundingInterestRates,
    SetFundingClamps,
    HaltTrading,
    InsertMarginTable,
    SetMarginTableIds,
    SetFeeRecipient,
    SetOpenInterestCaps,
    SetSubDeployers,
    SetMarginModes,
    SetDeployerFees,
    SetPerpAnnotation,
    DisableDex,
}

impl PerpKind {
    /// Every variant, in the order the docs list them.
    pub const ALL: [PerpKind; 16] = [
        PerpKind::RegisterAsset2,
        PerpKind::RegisterAsset,
        PerpKind::SetOracle,
        PerpKind::SetFundingMultipliers,
        PerpKind::SetFundingInterestRates,
        PerpKind::SetFundingClamps,
        PerpKind::HaltTrading,
        PerpKind::InsertMarginTable,
        PerpKind::SetMarginTableIds,
        PerpKind::SetFeeRecipient,
        PerpKind::SetOpenInterestCaps,
        PerpKind::SetSubDeployers,
        PerpKind::SetMarginModes,
        PerpKind::SetDeployerFees,
        PerpKind::SetPerpAnnotation,
        PerpKind::DisableDex,
    ];

    /// The key sent.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            PerpKind::RegisterAsset2 => "registerAsset2",
            PerpKind::RegisterAsset => "registerAsset",
            PerpKind::SetOracle => "setOracle",
            PerpKind::SetFundingMultipliers => "setFundingMultipliers",
            PerpKind::SetFundingInterestRates => "setFundingInterestRates",
            PerpKind::SetFundingClamps => "setFundingClamps",
            PerpKind::HaltTrading => "haltTrading",
            PerpKind::InsertMarginTable => "insertMarginTable",
            PerpKind::SetMarginTableIds => "setMarginTableIds",
            PerpKind::SetFeeRecipient => "setFeeRecipient",
            PerpKind::SetOpenInterestCaps => "setOpenInterestCaps",
            PerpKind::SetSubDeployers => "setSubDeployers",
            PerpKind::SetMarginModes => "setMarginModes",
            PerpKind::SetDeployerFees => "setDeployerFees",
            PerpKind::SetPerpAnnotation => "setPerpAnnotation",
            PerpKind::DisableDex => "disableDex",
        }
    }
}

/// One address let in to (`allowed`), or shut out of, one variant on the
/// dex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubDeployer {
    pub variant: PerpKind,
    pub user: Address,
    pub allowed: bool,
}

impl SubDeployer {
    /// `{"variant","user","allowed"}`.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("variant", self.variant.wire()).put("user", self.user.to_hex()).put("allowed", self.allowed)
    }
}
