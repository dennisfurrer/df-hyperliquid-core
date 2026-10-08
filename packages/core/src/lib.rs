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
//! - [`Api`]: one network's info and exchange endpoints over an [`Http`] wire, behind the rate-limit
//!   gate and meter ([`limit`]), so it runs native, in the browser and in Workers.
//!
//! Mainnet or testnet, named on every signature. `/exchange` is never retried.
#![deny(clippy::float_arithmetic)]

pub mod address;
pub mod api;
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
pub mod limit;
pub mod msgpack;
pub mod network;
pub mod nonce;
pub mod side;
pub mod signer;
pub mod signers;
pub mod system;
pub mod units;
pub mod user_signed;

pub use address::{Address, BadAddress};
pub use api::{Api, Clock};
pub use asset::{Asset, AssetKind, HIP3_ASSET_BASE, HIP3_DEX_SPAN, OUTCOME_ASSET_BASE, OutcomeSide, SPOT_ASSET_BASE};
pub use builder_fee::{BuilderFee, OUTCOME_BUILDER_CAP, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
pub use client::{
    Accepted, ApiError, ExchangeResponse, Http, Method, Request, Response, Status, Transport, TransportError, exchange,
    info, user_request,
};
pub use dec::{BadDec, Dec, Round};
pub use eip712::{Domain, Field, TypedStruct, digest, digest_of_wallet_json};
pub use figure::Figure;
pub use hex::BadHex;
pub use json::Json;
pub use keccak::keccak256;
pub use l1::{L1Action, connection_id, l1_digest, phantom_agent};
pub use limit::Spent;
pub use msgpack::{Map, Value};
pub use network::{L1_CHAIN_ID, MAINNET_USDC, Network, TESTNET_USDC, Token, USDC_INDEX, UnknownNetwork};
pub use nonce::NonceClock;
pub use side::Side;
#[cfg(feature = "sign")]
pub use signer::{LocalKey, recover, verify};
pub use signer::{SignError, Signature, Signer};
pub use signers::{ActionKind, Key, Signers, Why, may_sign};
pub use system::{HYPE_SYSTEM, is_system_address, system_address, system_token};
pub use units::{BadUnits, cut_units, exact_units};
pub use user_signed::{NonceKey, UserAction, UserSigned};
