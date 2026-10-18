//! The recorded frames under `fixtures/ws`, and what the live tests share to read them. Each
//! manifest says where its frames came from in its `source`, and which were trimmed in its
//! `notes` or its `source`; every other frame is the text as received.
//!
//! - `mainnet.json`: recorded from mainnet on 2026-10-08, subscriptions and info posts only.
//! - `testnet.json`: recorded from testnet the same day, with one refused action post.
//! - `portfolio.json`: one mainnet account's clearinghouse frames, recorded 2026-09-26.
//! - `docs.json`: the post-requests page's sample answer to an order.
//! - `vault.json`: the HLP vault's frames, recorded 2026-10-18: the vaults it leads, deposits into it
//!   and withdrawals from it, and its order history's snapshot.

use crate::account::history::Fill;
use crate::live::{Frame, decode};
use crate::markets::{Level, Side};
use crate::utils::{Address, Dec, Json};

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/testnet.json"));
const PORTFOLIO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/portfolio.json"));
const DOCS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/docs.json"));
const VAULT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/vault.json"));

/// The two accounts the mainnet recording followed on 2026-10-08.
pub const FIRST: &str = "0x42f1b36fb856769e075dfbf25938464fb3c92c70";
pub const SECOND: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";

fn frames(manifest: &str) -> Vec<(String, String)> {
    let manifest = Json::parse(manifest).unwrap();
    manifest
        .get("frames")
        .and_then(Json::as_object)
        .unwrap()
        .iter()
        .map(|(name, text)| (name.clone(), text.as_str().unwrap().to_owned()))
        .collect()
}

fn named(manifest: &str, name: &str) -> String {
    frames(manifest)
        .into_iter()
        .find(|(each, _)| each == name)
        .map(|(_, text)| text)
        .unwrap_or_else(|| panic!("no frame named {name}"))
}

pub fn mainnet(name: &str) -> String {
    named(MAINNET, name)
}

pub fn testnet(name: &str) -> String {
    named(TESTNET, name)
}

pub fn portfolio(name: &str) -> String {
    named(PORTFOLIO, name)
}

pub fn docs(name: &str) -> String {
    named(DOCS, name)
}

pub fn vault(name: &str) -> String {
    named(VAULT, name)
}

/// Every recorded frame, from every manifest.
pub fn all() -> Vec<(String, String)> {
    [MAINNET, TESTNET, PORTFOLIO, DOCS, VAULT].into_iter().flat_map(frames).collect()
}

pub fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn level(px: &str, sz: &str, n: u32) -> Level {
    Level { px: dec(px), sz: dec(sz), n }
}

pub fn read(text: &str) -> Frame {
    decode(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// The live fill SECOND took on mainnet, sent on `userFills` and again on `user`.
pub fn rebate() -> Fill {
    Fill {
        coin: "BTC".to_owned(),
        px: dec("80499.0"),
        sz: dec("0.00124"),
        side: Side::Sell,
        time: 1_791_480_468_944,
        start_position: dec("-11.86858"),
        dir: "Open Short".to_owned(),
        closed_pnl: Dec::ZERO,
        hash: "0xa36c5669b8ada710a4e60446200ab702061f004f53a0c5e2473501bc77a180fb".to_owned(),
        oid: 569_017_662_681,
        crossed: false,
        fee: dec("-0.002994"),
        tid: 167_376_889_247_413,
        cloid: Some("0x9c7328918712fb838952e96a69ea150d".to_owned()),
        fee_token: "USDC".to_owned(),
        builder_fee: None,
        liquidation: None,
        twap_id: None,
    }
}
