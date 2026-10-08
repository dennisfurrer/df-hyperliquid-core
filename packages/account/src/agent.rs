//! Agent wallets: `approveAgent`, the named agents an account has
//! (`extraAgents`), and what the venue knows an address as (`userRole`).
//!
//! An agent key signs L1 actions (orders, cancels, leverage, vault and
//! lending moves) for its master. It cannot move funds: every user-signed
//! action (a transfer, a withdrawal, an approval) needs the account's own
//! wallet, and the venue refuses one an agent signed.
//!
//! **There is no revoke action.** Approving the same name again replaces that
//! agent; approving a new unnamed one replaces the unnamed one.
//!
//! **Never approve an address that was an agent before:** actions it signed
//! then can be replayed once it is approved again.

use hl_core::{Address, ApiError, Field, NonceKey, Transport, UserAction, UserSigned, info, user_request};
use serde::Deserialize;

/// `approveAgent`, signed by the account's own wallet. `name` empty approves
/// the unnamed agent, signed and posted as the empty string as the SDK does.
///
/// There is no revoke: re-approving a name replaces that agent. Never
/// approve an address that was an agent before, since what it signed then
/// can be replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveAgent {
    agent: Address,
    name: String,
    nonce: u64,
}

impl ApproveAgent {
    #[must_use]
    pub fn new(agent: Address, name: &str, nonce: u64) -> ApproveAgent {
        ApproveAgent { agent, name: name.to_owned(), nonce }
    }

    #[must_use]
    pub const fn agent(&self) -> Address {
        self.agent
    }
}

impl UserAction for ApproveAgent {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("approveAgent", "HyperliquidTransaction:ApproveAgent", NonceKey::Nonce, self.nonce)
            .field("agentAddress", Field::Address(self.agent))
            .field("agentName", Field::String(&self.name))
    }
}

/// `{"type":"extraAgents","user":..}`.
#[must_use]
pub fn extra_agents_request(user: Address) -> String {
    user_request("extraAgents", user)
}

/// The account's named agents. The venue has answered both `null` and `[]`
/// for an account with none; both read as none.
pub async fn extra_agents(transport: &impl Transport, user: Address) -> Result<Vec<Agent>, ApiError> {
    let agents: Option<Vec<Agent>> = info(transport, extra_agents_request(user)).await?;
    Ok(agents.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    #[serde(default)]
    pub name: String,
    pub address: Address,
    /// Milliseconds; after it the agent signs nothing.
    #[serde(default)]
    pub valid_until: Option<u64>,
}

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
