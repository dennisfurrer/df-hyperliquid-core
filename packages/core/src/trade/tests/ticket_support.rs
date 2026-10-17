//! Each ticket check down to the exact JSON it would sign. The markets: BTC (asset 0, five size
//! decimals, 40x), ETH (1, four), `xyz:TSLA` (110001, three, 10x, isolated only), PURR/USDC (10000,
//! none) and `@107` (10107, two). A market order is bounded from 84000.5, one percent away.

use crate::trade::*;

use crate::utils::sign::L1Action;
use crate::utils::{Address, Asset, Dec};

use crate::account::orders::FrontendOrder;
use crate::trade::builder::BuilderSetting;
use crate::trade::orders::{Side, Tif};

pub fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

pub fn btc() -> Market {
    Market::perp(Asset::from_id(0), 5, 40, true).unwrap()
}

pub fn eth() -> Market {
    Market::perp(Asset::from_id(1), 4, 25, true).unwrap()
}

pub fn tsla() -> Market {
    Market::perp(Asset::from_id(110_001), 3, 10, false).unwrap()
}

pub fn hype() -> Market {
    Market::spot(Asset::from_id(10_107), 2).unwrap()
}

pub fn one_percent() -> Dec {
    dec("0.01")
}

pub fn market(side: Side, size: &str) -> OrderIntent {
    OrderIntent::market(side, Amount::Size(dec(size)), dec("84000.5"), one_percent())
}

pub fn limit(side: Side, price: &str, size: &str, tif: Tif) -> OrderIntent {
    OrderIntent::limit(side, Amount::Size(dec(size)), dec(price), tif)
}

pub fn wire(placed: Result<impl L1Action, Refusal>) -> String {
    placed.unwrap().wire()
}

pub fn terminal() -> Address {
    "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843".parse().unwrap()
}

pub fn at_88_percent() -> BuilderSetting {
    BuilderSetting::share_of_cap(terminal(), 88).unwrap()
}

/// A resting order as `frontendOpenOrders` writes one, in the venue's own
/// words: `B` a buy, a stop with its `triggerPx`, a position's at `"0.0"`.
pub fn resting(oid: u64, side: &str, price: &str, size: &str, kind: &str, trigger: Option<&str>) -> FrontendOrder {
    let (is_trigger, trigger_px, tif, reduce) = match trigger {
        Some(at) => (true, at, "null", true),
        None => (false, "0.0", r#""Gtc""#, false),
    };
    serde_json::from_str(&format!(
        r#"{{"coin":"BTC","side":"{side}","limitPx":"{price}","sz":"{size}","oid":{oid},"timestamp":1790415847986,"triggerCondition":"N/A","isTrigger":{is_trigger},"triggerPx":"{trigger_px}","children":[],"isPositionTpsl":false,"reduceOnly":{reduce},"orderType":"{kind}","origSz":"{size}","tif":{tif},"cloid":null}}"#
    ))
    .unwrap()
}
