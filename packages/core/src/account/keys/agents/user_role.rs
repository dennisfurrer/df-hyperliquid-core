//! `userRole`: what the venue knows an address as, and for an agent or a subaccount, the account
//! behind it.

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::Address;
use serde::Deserialize;

/// `{"type":"userRole","user":..}`.
#[must_use]
pub fn role_request(user: Address) -> String {
    user_request("userRole", user)
}

pub async fn role(transport: &impl Transport, user: Address) -> Result<Role, ApiError> {
    info(transport, role_request(user)).await
}

/// What the venue knows an address as.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawRole")]
pub enum Role {
    User,
    /// An address the venue has never seen.
    Missing,
    Vault,
    /// An agent, and the account it signs for.
    Agent {
        user: Address,
    },
    /// A subaccount, and its master.
    SubAccount {
        master: Address,
    },
    /// A role this build does not know, or a known one whose data did not
    /// read, kept as spelled.
    Other(String),
}

impl Role {
    /// The account an agent signs for.
    #[must_use]
    pub const fn agent_of(&self) -> Option<Address> {
        match self {
            Role::Agent { user } => Some(*user),
            _ => None,
        }
    }

    /// A subaccount's master.
    #[must_use]
    pub const fn master(&self) -> Option<Address> {
        match self {
            Role::SubAccount { master } => Some(*master),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct RawRole {
    #[serde(default)]
    role: String,
    #[serde(default)]
    data: Option<RawRoleData>,
}

#[derive(Deserialize)]
struct RawRoleData {
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    master: Option<String>,
}

impl From<RawRole> for Role {
    fn from(raw: RawRole) -> Role {
        let named = |pick: fn(&RawRoleData) -> Option<&String>| -> Option<Address> {
            raw.data.as_ref().and_then(pick).and_then(|text| text.parse().ok())
        };
        match raw.role.as_str() {
            "user" => Role::User,
            "missing" => Role::Missing,
            "vault" => Role::Vault,
            "agent" => {
                named(|data| data.user.as_ref()).map_or(Role::Other(raw.role.clone()), |user| Role::Agent { user })
            }
            "subAccount" => named(|data| data.master.as_ref())
                .map_or(Role::Other(raw.role.clone()), |master| Role::SubAccount { master }),
            _ => Role::Other(raw.role.clone()),
        }
    }
}

#[cfg(test)]
#[path = "tests/user_role.rs"]
mod tests;
