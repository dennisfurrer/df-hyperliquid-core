//! Signing primitives: keccak, msgpack, EIP-712, L1 actions and user-signed actions.
pub mod eip712;
pub mod keccak;
pub mod l1;
pub mod msgpack;
pub mod user_signed;

pub use eip712::{Domain, Field, TypedStruct, digest, digest_of_wallet_json};
pub use keccak::keccak256;
pub use l1::{L1Action, connection_id, l1_digest, phantom_agent};
pub use msgpack::{Map, Value};
pub use user_signed::{NonceKey, UserAction, UserSigned};
