//! Running a validator: registering it, changing its profile and unregistering it
//! (`CValidatorAction`), and its signer jailing or unjailing itself (`CSignerAction`). Both are L1
//! actions. Neither is in the API docs; the shapes are the ones `@nktkas/hyperliquid` and
//! Hyperliquid's Python SDK both send. Keys are snake case, as the venue takes them.

use std::net::IpAddr;

use crate::utils::Address;
use crate::utils::sign::{L1Action, Map, Value};

/// `{"Ip":..}`, the node address as sent. An IPv6 address is written in its
/// short form, `2001:db8::1`.
fn node_ip(ip: IpAddr) -> Map {
    Map::new().put("Ip", ip.to_string())
}

/// A new validator's profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorProfile {
    /// The validator node's IP address.
    pub node_ip: IpAddr,
    /// The validator's name.
    pub name: String,
    /// The validator's description.
    pub description: String,
    /// Whether the validator refuses delegations.
    pub delegations_disabled: bool,
    /// The commission, as the integer the venue takes.
    pub commission_bps: u64,
    /// The address that signs for the validator node.
    pub signer: Address,
}

impl ValidatorProfile {
    /// `{"node_ip","name","description","delegations_disabled","commission_bps","signer"}`.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new()
            .put("node_ip", node_ip(self.node_ip))
            .put("name", self.name.clone())
            .put("description", self.description.clone())
            .put("delegations_disabled", self.delegations_disabled)
            .put("commission_bps", self.commission_bps)
            .put("signer", self.signer.to_hex())
    }
}

/// A change to a validator's profile. Every `None` is sent as null and
/// leaves that field as it is; `unjailed` is always sent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileChange {
    /// The new node IP address (`node_ip`, sent as `{"Ip":..}`).
    pub node_ip: Option<IpAddr>,
    /// The new name.
    pub name: Option<String>,
    /// The new description.
    pub description: Option<String>,
    /// Whether the validator is unjailed after the change.
    pub unjailed: bool,
    /// Whether the validator refuses delegations (`disable_delegations`).
    pub disable_delegations: Option<bool>,
    /// The new commission (`commission_bps`), as the integer the venue takes.
    pub commission_bps: Option<u64>,
    /// The new address that signs for the validator node.
    pub signer: Option<Address>,
}

impl ProfileChange {
    /// `{"node_ip","name","description","unjailed","disable_delegations","commission_bps","signer"}`.
    #[must_use]
    pub fn wire(&self) -> Map {
        fn or_nil<T>(value: Option<T>, wire: impl FnOnce(T) -> Value) -> Value {
            value.map_or(Value::Nil, wire)
        }
        Map::new()
            .put("node_ip", or_nil(self.node_ip, |ip| node_ip(ip).into()))
            .put("name", or_nil(self.name.clone(), Value::from))
            .put("description", or_nil(self.description.clone(), Value::from))
            .put("unjailed", self.unjailed)
            .put("disable_delegations", or_nil(self.disable_delegations, Value::from))
            .put("commission_bps", or_nil(self.commission_bps, Value::from))
            .put("signer", or_nil(self.signer, |signer| signer.to_hex().into()))
    }
}

/// `{"type":"CValidatorAction",<variant>:..}`, sent by the validator's own
/// key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CValidatorAction {
    /// `register`: a new validator with its profile, whether it starts
    /// unjailed, and its initial self-stake in HYPE wei (8 decimals, as
    /// [`hype_wei`](crate::earn::staking::hype_wei) converts).
    Register {
        /// The new validator's profile.
        profile: ValidatorProfile,
        /// Whether the validator starts unjailed.
        unjailed: bool,
        /// The initial self-stake (`initial_wei`), in HYPE wei: 8 decimals.
        initial_wei: u64,
    },
    /// `changeProfile`.
    ChangeProfile(ProfileChange),
    /// `unregister`: the key is sent, with null.
    Unregister,
}

impl L1Action for CValidatorAction {
    fn action(&self) -> Map {
        let map = Map::new().put("type", "CValidatorAction");
        match self {
            CValidatorAction::Register { profile, unjailed, initial_wei } => map.put(
                "register",
                Map::new().put("profile", profile.wire()).put("unjailed", *unjailed).put("initial_wei", *initial_wei),
            ),
            CValidatorAction::ChangeProfile(change) => map.put("changeProfile", change.wire()),
            CValidatorAction::Unregister => map.put("unregister", Value::Nil),
        }
    }
}

/// `{"type":"CSignerAction",<variant>:null}`, sent by a validator's signer
/// key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CSignerAction {
    /// `jailSelf`.
    JailSelf,
    /// `unjailSelf`.
    UnjailSelf,
}

impl L1Action for CSignerAction {
    fn action(&self) -> Map {
        let variant = match self {
            CSignerAction::JailSelf => "jailSelf",
            CSignerAction::UnjailSelf => "unjailSelf",
        };
        Map::new().put("type", "CSignerAction").put(variant, Value::Nil)
    }
}

#[cfg(test)]
#[path = "tests/validator.rs"]
mod tests;
