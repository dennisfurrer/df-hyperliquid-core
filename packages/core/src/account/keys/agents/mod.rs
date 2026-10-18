//! API wallets (agents): `approveAgent`, an account's named agents (`extraAgents`) and what the
//! venue knows an address as (`userRole`). An agent signs L1 actions for its master and never
//! moves funds; there is no revoke, so an address that was an agent is never approved again.

pub mod approve;
pub mod fresh;
pub mod listed;
pub mod name;
pub mod user_role;

pub use approve::{ApproveAgent, BadExpiry, MAX_AGENT_VALIDITY_MS};
pub use fresh::{Reused, fresh_agent};
pub use listed::{Agent, extra_agents, extra_agents_request};
pub use name::{AgentName, BadAgentName, LONGEST_AGENT_NAME, split_valid_until};
pub use user_role::{Role, role, role_request};
