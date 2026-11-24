//! Aligned quote assets: a validator's vote on the risk-free rate (`validatorL1Stream`) and an
//! AQAv2 deployer authorizing its role from its staking account (`authorizeAqav2Role`). Both are
//! L1 actions.

use crate::deploy::error::BadDeploy;
use crate::utils::Dec;
use crate::utils::sign::{L1Action, Map};

/// `{"type":"validatorL1Stream","riskFreeRate"}`: a validator's vote on the
/// risk-free rate for aligned quote assets, `0.04` for 4%. The rate used is
/// the stake-weighted median of the votes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatorL1Stream {
    risk_free_rate: Dec,
}

impl ValidatorL1Stream {
    /// Refuses a negative rate.
    pub fn new(risk_free_rate: Dec) -> Result<Self, BadDeploy> {
        if risk_free_rate.is_negative() {
            return Err(BadDeploy::OutOfRange {
                what: "a risk-free rate",
                least: "0",
                most: "any",
                value: risk_free_rate.to_wire(),
            });
        }
        Ok(ValidatorL1Stream { risk_free_rate })
    }

    /// The rate voted for, as a fraction.
    #[must_use]
    pub const fn risk_free_rate(self) -> Dec {
        self.risk_free_rate
    }
}

impl L1Action for ValidatorL1Stream {
    fn action(&self) -> Map {
        Map::new().put("type", "validatorL1Stream").put("riskFreeRate", self.risk_free_rate.to_wire())
    }
}

/// The two AQAv2 deployer roles. Each stakes 500k HYPE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AqaRole {
    /// `technical`: runs minting, redemption and cross-chain transfers.
    Technical,
    /// `treasury`: names the treasury address that shares the AQA rate with
    /// the protocol.
    Treasury,
}

impl AqaRole {
    /// The name sent.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            AqaRole::Technical => "technical",
            AqaRole::Treasury => "treasury",
        }
    }
}

/// `{"type":"authorizeAqav2Role","token","role"}`: sent from a deployer's
/// staking account to take an AQAv2 role for `token` (0 for USDC).
/// Activation then goes to a validator vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizeAqav2Role {
    pub token: u32,
    pub role: AqaRole,
}

impl L1Action for AuthorizeAqav2Role {
    fn action(&self) -> Map {
        Map::new().put("type", "authorizeAqav2Role").put("token", self.token).put("role", self.role.wire())
    }
}
