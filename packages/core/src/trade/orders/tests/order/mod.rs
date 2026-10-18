//! The order family's actions against other implementations' signing vectors and the venue's rules:
//!
//! - `nktkas.rs`: `@nktkas/hyperliquid` 0.33.2's vectors, byte for byte.
//! - `python.rs`: the official Python SDK's `signing_test.py` order vectors.
//! - `wire.rs`: a market order's tif, and the keys and spellings the venue hashes, which no vector pins.
//! - `refused.rs`: what the venue would refuse, known before signing.

mod nktkas;
mod python;
mod refused;
mod wire;

use crate::account::LocalKey;
use crate::trade::builder::fee::BuilderFee;
use crate::trade::orders::*;
use crate::utils::{Address, Asset, Dec};

/// The key `hyperliquid-python-sdk` signs its tests with.
const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
/// The builder, checksummed as the vectors name it.
const BUILDER: &str = "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843";

fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn asset(id: u32) -> Asset {
    Asset::from_id(id)
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn limit(id: u32, side: Side, price: &str, size: &str, tif: Tif) -> Order {
    Order::limit(asset(id), side, dec(price), dec(size), tif)
}

fn through(order: Order, builder: &str, fee: u64) -> PlaceOrder {
    PlaceOrder::one(order).through(Builder::new(address(builder), BuilderFee::from_tenths(fee)))
}
