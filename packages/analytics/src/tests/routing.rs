//! hip4-tui's `tui/src/hip3/tests/mod.rs` on the builders that route perps, with its recorded
//! answers: two builders' day files for 2026-09-26 from Hyperliquid's bucket (stored as sent), one
//! builder with no file and one read that failed. The perp volumes are hl-hydromancer's sums of the
//! same files (Python's `decimal`); the shares were worked out from them.

use hl_core::{Address, Dec};
use hl_hydromancer::{BuilderDay, days};

use super::*;
use crate::builders::Board;

const WIDE: &[u8] = include_bytes!("../../fixtures/builder_fills_0c8d970c_20260926.csv.lz4");
const SMALL: &[u8] = include_bytes!("../../fixtures/builder_fills_8d62d300_20260926.csv.lz4");
const WIDE_BUILDER: &str = "0x0c8d970c462726e014ad36f6c5a63e99db48a8e7";
const SMALL_BUILDER: &str = "0x8d62d3000ef0639d1fc9667d06be7bb98d9993f5";
const OUTCOME: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";
const DFU: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";
const DOCUMENTED: &str = include_str!("../../fixtures/dashboard_volume_daily.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn day(bytes: &[u8], builder: &str) -> BuilderDay {
    let csv = days::unframe(bytes).unwrap();
    days::read_day(address(builder), "20260926", &String::from_utf8(csv).unwrap()).unwrap()
}

fn came() -> Vec<(Address, Result<Option<BuilderDay>, String>)> {
    vec![
        (address(SMALL_BUILDER), Ok(Some(day(SMALL, SMALL_BUILDER)))),
        (address(WIDE_BUILDER), Ok(Some(day(WIDE, WIDE_BUILDER)))),
        (address(OUTCOME), Ok(None)),
        (address(DFU), Err("the bucket timed out".to_owned())),
    ]
}

#[test]
fn the_builders_are_ranked_by_perp_volume_and_the_rest_counted() {
    let routing = Routing::of("20260926", came());
    assert_eq!(routing.date, "20260926");
    let order: Vec<Address> = routing.days.iter().map(|day| day.builder).collect();
    assert_eq!(order, [address(WIDE_BUILDER), address(SMALL_BUILDER)], "the busier builder first");
    assert_eq!((routing.asked, routing.quiet, routing.failed), (4, 1, 1));
    assert_eq!(routing.error.as_deref(), Some("0x6967d7acd1ec5f210e9590666498e0cc5d13a843: the bucket timed out"));
}

#[test]
fn a_share_is_of_the_perp_volume_every_ranked_builder_routed() {
    let routing = Routing::of("20260926", came());
    assert_eq!(routing.total(), dec("28989.832888"), "24,339.321586 and 4,650.511302");
    let shares: Vec<u32> = routing.days.iter().map(|day| routing.share(day)).collect();
    assert_eq!(shares, [8_395, 1_604]);
    assert!((9_998..=10_000).contains(&shares.iter().sum::<u32>()), "shares make the whole");
    assert_eq!(
        routing.fees(),
        routing.days.iter().fold(Dec::ZERO, |sum, day| sum.checked_add(day.perps().fees).unwrap())
    );
    assert_eq!(routing.days[1].perps().fees, dec("1.860195"), "what the small builder's perps paid it");
}

#[test]
fn a_builder_with_no_perp_fills_that_day_is_quiet_not_ranked() {
    let outcomes_only = days::read_day(
        address(OUTCOME),
        "20260926",
        "time,user,coin,side,px,sz,crossed,special_trade_type,tif,is_trigger,counterparty,closed_pnl,twap_id,builder_fee\n\
         1,0x0000000000000000000000000000000000000001,#12270,B,0.5,10,true,,Gtc,false,0x00,0,,0.044\n",
    )
    .unwrap();
    let routing = Routing::of("20260926", vec![(address(OUTCOME), Ok(Some(outcomes_only)))]);
    assert!(routing.days.is_empty());
    assert_eq!((routing.asked, routing.quiet, routing.failed), (1, 1, 0));
    assert_eq!(routing.total(), Dec::ZERO);
}

#[test]
fn the_builders_asked_about_are_the_callers_the_curated_list_and_the_outcome_board() {
    let stranger = "0x00000000000000000000000000000000000000aa";
    let answer = DOCUMENTED.replace(OUTCOME, stranger);
    let board = Board::of(&serde_json::from_str(&answer).unwrap()).unwrap();
    let asked = candidates(&[address(DFU)], Some(&board), 25);
    assert_eq!(asked[0], address(DFU), "the caller's own first");
    assert_eq!(asked.len(), 60, "the 59 curated, DFU among them once, and the board's one stranger");
    assert_eq!(asked[59], address(stranger));
    assert_eq!(candidates(&[], None, 25).len(), 59);
    assert_eq!(candidates(&[], Some(&board), 0).len(), 59, "a top of none adds nobody");
}

#[test]
fn the_newest_day_with_a_file_is_the_utc_day_before() {
    assert_eq!(yesterday(At::parse("2026-09-27T00:05:00Z").unwrap()), "20260926");
    assert_eq!(yesterday(At::parse("2027-01-01T12:00:00Z").unwrap()), "20261231");
}
