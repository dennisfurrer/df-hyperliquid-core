//! Sub-deployers: what a venue's deployer may let another address send ([`Permission`]), and one
//! address let in to or shut out of it ([`Grant`]).

use crate::utils::Address;
use crate::utils::sign::Map;

/// What a sub-deployer may do for a venue: a `setSubDeployers` grant's
/// `variant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    RegisterStandalone,
    RegisterQuestion,
    RegisterAndAssociate,
    SettleOutcome,
    /// Covers `settleQuestion2`, under the older name.
    SettleQuestion,
}

impl Permission {
    pub const ALL: [Permission; 5] = [
        Permission::RegisterStandalone,
        Permission::RegisterQuestion,
        Permission::RegisterAndAssociate,
        Permission::SettleOutcome,
        Permission::SettleQuestion,
    ];

    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Permission::RegisterStandalone => "registerStandaloneOutcomeFromTemplate",
            Permission::RegisterQuestion => "registerQuestionFromTemplate",
            Permission::RegisterAndAssociate => "registerAndAssociateNamedOutcomeFromTemplate",
            Permission::SettleOutcome => "settleOutcome",
            Permission::SettleQuestion => "settleQuestion",
        }
    }
}

/// One address let in to, or shut out of, one [`Permission`] on the venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    pub permission: Permission,
    pub user: Address,
    pub allowed: bool,
}

impl Grant {
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("variant", self.permission.wire()).put("user", self.user.to_hex()).put("allowed", self.allowed)
    }
}
