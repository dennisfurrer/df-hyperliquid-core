//! Signing primitives: keccak, msgpack, EIP-712, L1 actions, user-signed actions and multi-sig.
pub mod breakdown;
pub mod eip712;
pub mod keccak;
pub mod l1;
pub mod msgpack;
pub mod multi_sig;
pub mod user_signed;

pub use breakdown::Breakdown;
pub use eip712::{Domain, Field, TypedStruct, digest, digest_of_wallet_json};
pub use keccak::{keccak256, keccak256_of_two};
pub use l1::{L1Action, agent_digest, connection_id, connection_id_expiring, l1_digest, phantom_agent};
pub use msgpack::{Map, Value};
pub use multi_sig::MultiSig;
pub use user_signed::{NonceKey, UserAction, UserSigned};
