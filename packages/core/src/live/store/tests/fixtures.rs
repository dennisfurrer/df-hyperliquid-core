//! Recorded data the tests fold: websocket frames from mainnet (2026-10-08 and 2026-09-26) and
//! `/info` answers from mainnet (2026-10-08). Nothing here is edited; a test that needs a frame
//! the venue did not send builds it from a recorded one and says what it changed.

use std::sync::OnceLock;

use crate::live::{Frame, decode};
use crate::utils::{Address, Dec};
use serde::de::DeserializeOwned;

const WS_MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/mainnet.json"));
const WS_HIP4_TUI: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/hip4-tui.json"));

pub const L2_BOOK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/l2_book.json"));
pub const RECENT_TRADES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/recent_trades.json"));
pub const CANDLE_SNAPSHOT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/candle_snapshot.json"));
pub const USER_FILLS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills.json"));
pub const FRONTEND_OPEN_ORDERS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/frontend_open_orders.json"));
pub const ALL_MIDS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/all_mids.json"));

pub const USER_FUNDING: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/user_funding_mainnet.json"));
pub const PERP_META: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));

pub const PERP_DEXS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_dexs_mainnet.json"));
pub const ALL_PERP_METAS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/all_perp_metas_mainnet.json"));
pub const PARA: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/meta_and_asset_ctxs_para_mainnet.json"));

pub const SPOT_META_TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/spot_meta_testnet.json"));

/// A unified account (`0x114dc2...`) and its subaccount, and a standard one (`0x677d83...`),
/// each read within seconds; the prices read straight after; and a portfolio-margin account's
/// spot (2026-10-02).
pub const SPOT_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_unified.json"));
pub const PERP_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_unified.json"));
pub const PERP_UNIFIED_XYZ: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_unified_xyz.json"));
pub const PORTFOLIO_UNIFIED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/portfolio_unified.json"));
pub const SUBS_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/subs_unified.json"));
pub const SPOT_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_standard.json"));
pub const PERP_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_standard.json"));
pub const PERP_STANDARD_XYZ: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_standard_xyz.json"));
pub const VAULTS_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/vaults_standard.json"));
pub const PORTFOLIO_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/portfolio_standard.json"));
pub const PRICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/prices.json"));
pub const SPOT_PORTFOLIO_MARGIN: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_portfolio_margin.json"));

/// The accounts the 2026-10-08 frames follow.
pub const FIRST: &str = "0x42f1b36fb856769e075dfbf25938464fb3c92c70";
pub const SECOND: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";
/// The account the 2026-09-26 frames follow.
pub const TUI: &str = "0x8d1548a936241928cef288a1c377919a5d511f7a";
/// The recorded unified account, its subaccount, and the standard account.
pub const UNIFIED: &str = "0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d";
pub const UNIFIED_SUB: &str = "0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f";
pub const STANDARD: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";

pub fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn read<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {}", &text[..text.len().min(200)]))
}

/// A recorded frame's text, by its name in a manifest.
fn text(manifest: &serde_json::Value, name: &str) -> String {
    manifest["frames"][name].as_str().unwrap_or_else(|| panic!("no frame named {name}")).to_owned()
}

/// A manifest, read once.
fn manifest(cell: &'static OnceLock<serde_json::Value>, text: &str) -> &'static serde_json::Value {
    cell.get_or_init(|| read(text))
}

static MAINNET: OnceLock<serde_json::Value> = OnceLock::new();
static HIP4_TUI: OnceLock<serde_json::Value> = OnceLock::new();

/// A frame recorded from mainnet, as received.
pub fn sent(name: &str) -> String {
    text(manifest(&MAINNET, WS_MAINNET), name)
}

/// A frame recorded from mainnet, read.
pub fn frame(name: &str) -> Frame {
    decode(&sent(name)).unwrap()
}

/// A frame from the 2026-09-26 recording, read.
pub fn tui_frame(name: &str) -> Frame {
    decode(&text(manifest(&HIP4_TUI, WS_HIP4_TUI), name)).unwrap()
}

/// A recorded frame with `change` made to its JSON first: for a frame the venue did not send.
pub fn changed(name: &str, change: impl FnOnce(&mut serde_json::Value)) -> Frame {
    let mut json: serde_json::Value = read(&sent(name));
    change(&mut json);
    decode(&json.to_string()).unwrap()
}

/// What an info post answered, as written.
pub fn answered(name: &str) -> String {
    let Frame::Post(reply) = frame(name) else { panic!("{name} is not a post") };
    let crate::live::Answer::Info { data, .. } = reply.answer else { panic!("{name} is not an info answer") };
    data
}

/// Every perp dex with its meta, as `Dexes::new` lines them up.
pub fn dexes() -> crate::markets::dexes::Dexes {
    crate::markets::dexes::Dexes::new(read(PERP_DEXS), read(ALL_PERP_METAS)).unwrap()
}
