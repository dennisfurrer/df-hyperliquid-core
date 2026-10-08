//! Recorded data the tests fold, read where the crates that recorded it keep it:
//!
//! - hl-ws's frames (`ws/src/tests/fixtures`): mainnet, 2026-10-08, and hip4-tui's from
//!   2026-09-26;
//! - hl-trade's, hl-perps', hl-hip3's and hl-account's `/info` answers, mainnet, 2026-10-08.
//!
//! Nothing here is edited. A test that needs a frame the venue did not send builds it from a
//! recorded one and says what it changed.

use std::sync::OnceLock;

use hl_core::{Address, Dec};
use hl_ws::{Frame, decode};
use serde::de::DeserializeOwned;

const WS_MAINNET: &str = include_str!("../../../ws/src/tests/fixtures/mainnet.json");
const WS_HIP4_TUI: &str = include_str!("../../../ws/src/tests/fixtures/hip4-tui.json");

pub const L2_BOOK: &str = include_str!("../../../trade/fixtures/l2_book.json");
pub const RECENT_TRADES: &str = include_str!("../../../trade/fixtures/recent_trades.json");
pub const CANDLE_SNAPSHOT: &str = include_str!("../../../trade/fixtures/candle_snapshot.json");
pub const USER_FILLS: &str = include_str!("../../../trade/fixtures/user_fills.json");
pub const FRONTEND_OPEN_ORDERS: &str = include_str!("../../../trade/fixtures/frontend_open_orders.json");
pub const ALL_MIDS: &str = include_str!("../../../trade/fixtures/all_mids.json");

pub const USER_FUNDING: &str = include_str!("../../../perps/src/tests/fixtures/user_funding_mainnet.json");
pub const PERP_META: &str = include_str!("../../../perps/src/tests/fixtures/meta_mainnet.json");

pub const PERP_DEXS: &str = include_str!("../../../hip3/src/tests/fixtures/perp_dexs_mainnet.json");
pub const ALL_PERP_METAS: &str = include_str!("../../../hip3/src/tests/fixtures/all_perp_metas_mainnet.json");
pub const PARA: &str = include_str!("../../../hip3/src/tests/fixtures/meta_and_asset_ctxs_para_mainnet.json");

pub const SPOT_META_TESTNET: &str = include_str!("../../../spot/src/tests/fixtures/spot_meta_testnet.json");

/// hl-account's unified account (`0x114dc2…`) and its subaccount, and a standard one
/// (`0x677d83…`), each read within seconds; the prices read straight after; and a
/// portfolio-margin account's spot as atlas recorded it (2026-10-02).
pub const SPOT_UNIFIED: &str = include_str!("../../../account/fixtures/spot_unified.json");
pub const PERP_UNIFIED: &str = include_str!("../../../account/fixtures/perp_unified.json");
pub const PERP_UNIFIED_XYZ: &str = include_str!("../../../account/fixtures/perp_unified_xyz.json");
pub const PORTFOLIO_UNIFIED: &str = include_str!("../../../account/fixtures/portfolio_unified.json");
pub const SUBS_UNIFIED: &str = include_str!("../../../account/fixtures/subs_unified.json");
pub const SPOT_STANDARD: &str = include_str!("../../../account/fixtures/spot_standard.json");
pub const PERP_STANDARD: &str = include_str!("../../../account/fixtures/perp_standard.json");
pub const PERP_STANDARD_XYZ: &str = include_str!("../../../account/fixtures/perp_standard_xyz.json");
pub const VAULTS_STANDARD: &str = include_str!("../../../account/fixtures/vaults_standard.json");
pub const PORTFOLIO_STANDARD: &str = include_str!("../../../account/fixtures/portfolio_standard.json");
pub const PRICES: &str = include_str!("../../../account/fixtures/prices.json");
pub const SPOT_PORTFOLIO_MARGIN: &str = include_str!("../../../account/fixtures/spot_portfolio_margin.json");

/// The accounts hl-ws's recordings follow.
pub const FIRST: &str = "0x42f1b36fb856769e075dfbf25938464fb3c92c70";
pub const SECOND: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";
/// The account hip4-tui's recording follows.
pub const TUI: &str = "0x8d1548a936241928cef288a1c377919a5d511f7a";
/// hl-account's accounts.
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

/// A frame hip4-tui recorded, read.
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
    let hl_ws::Answer::Info { data, .. } = reply.answer else { panic!("{name} is not an info answer") };
    data
}

/// Every perp dex with its meta, as hl-hip3 lines them up.
pub fn dexes() -> hl_hip3::Dexes {
    hl_hip3::Dexes::new(read(PERP_DEXS), read(ALL_PERP_METAS)).unwrap()
}
