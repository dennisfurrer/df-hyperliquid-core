//! Who may sign for an account: signatures and the [`Signer`] trait, which key may sign which
//! action, API wallets (agents), turning the account into a multi-sig one, and the builder fee an
//! account approves and whether a builder can be paid.

pub mod agents;
pub mod builder;
#[cfg(feature = "sign")]
pub(crate) mod curve;
pub mod multi_sig;
#[cfg(feature = "sign")]
mod recovery;
pub mod signer;
pub mod signers;

pub use agents::{
    Agent, AgentName, ApproveAgent, BadAgentName, BadExpiry, LONGEST_AGENT_NAME, MAX_AGENT_VALIDITY_MS, Reused, Role,
    extra_agents, extra_agents_request, fresh_agent, role, role_request, split_valid_until,
};
pub use builder::{
    ApproveBuilderFee, BUILDER_MINIMUM, BUILDER_MODE, NotReady, Readiness, approved_builders,
    approved_builders_request, builder_readiness, max_builder_fee, max_builder_fee_request, readiness,
};
pub use multi_sig::{BadSigners, ConvertToMultiSigUser, MOST_SIGNERS};
#[cfg(feature = "sign")]
pub use signer::{LocalKey, address_of_public_key, recover, verify};
pub use signer::{SignError, Signature, Signer};
pub use signers::{ActionKind, Key, Refused, Signers, Why, may_sign};
