//! Hyperliquid backbone. Every other `hl-*` crate builds on this one and on nothing else in the family
//! (except the product crates, which may also use `hl-trade`).
//!
//! Owns:
//! - vocabulary: addresses, exact decimals (never floats), spot tokens, asset ids, builder fees,
//!   integer amounts, and the few floats the venue writes, kept as their digits;
//! - wire: msgpack in insertion order for L1 actions, and JSON that keeps the same order;
//! - signing: L1 actions (phantom agent) and user-signed EIP-712 actions, the typed data a browser
//!   wallet signs, recovery, behind a `Signer` trait;
//! - nonces: one monotonic millisecond clock per account;
//! - the info and exchange client over a `Transport` trait, so it runs native, in the browser and in Workers.
//!
//! Mainnet only. `/exchange` is never retried.
#![deny(clippy::float_arithmetic)]

pub mod address;
pub mod asset;
pub mod body;
pub mod builder_fee;
pub mod client;
pub mod dec;
pub mod eip712;
pub mod figure;
pub mod hex;
pub mod json;
pub mod keccak;
pub mod l1;
pub mod mainnet;
pub mod msgpack;
pub mod nonce;
pub mod signer;
pub mod units;
pub mod user_signed;

pub use address::{Address, BadAddress};
pub use asset::{Asset, AssetKind, HIP3_ASSET_BASE, HIP3_DEX_SPAN, OUTCOME_ASSET_BASE, OutcomeSide, SPOT_ASSET_BASE};
pub use builder_fee::{BuilderFee, OUTCOME_BUILDER_CAP, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
pub use client::{
    Accepted, ApiError, ExchangeResponse, Status, Transport, TransportError, exchange, info, user_request,
};
pub use dec::{BadDec, Dec, Round};
pub use eip712::{Domain, Field, TypedStruct, digest, digest_of_wallet_json};
pub use figure::Figure;
pub use hex::BadHex;
pub use json::Json;
pub use keccak::keccak256;
pub use l1::{L1Action, connection_id, l1_digest, phantom_agent};
pub use mainnet::{
    AGENT_SOURCE, API_URL, HYPERLIQUID_CHAIN, L1_CHAIN_ID, SIGNATURE_CHAIN_ID, SIGNATURE_CHAIN_ID_HEX, Token, USDC,
};
pub use msgpack::{Map, Value};
pub use nonce::NonceClock;
pub use signer::{LocalKey, SignError, Signature, Signer, recover, verify};
pub use units::{BadUnits, cut_units, exact_units};
pub use user_signed::{NonceKey, UserAction, UserSigned};
