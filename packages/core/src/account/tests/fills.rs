//! Live answers, mainnet, 2026-10-08, from one account: fills of each direction and fee token, one
//! market sell's fills by time and aggregated, three TWAP slices and its fee schedule. The builder
//! fill is the venue's own example from its info endpoint docs, since no recorded fill paid one.

use crate::utils::{Address, Dec};

use super::*;
use crate::trade::order::Side;
use crate::trade::tests_support::{Recorded, block_on};

const FILLS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills.json"));
const BY_TIME: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills_by_time.json"));
const AGGREGATED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills_by_time_aggregated.json"));
const SLICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_twap_slice_fills.json"));
const FEES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fees.json"));

/// The info endpoint page's perp fill, comments taken out.
const DOCS_FILL: &str = r#"{"closedPnl":"0.0","coin":"AVAX","crossed":false,"dir":"Open Long","hash":"0xa166e3fa63c25663024b03f2e0da011a00307e4017465df020210d3d432e7cb8","oid":90542681,"px":"18.435","side":"B","startPosition":"26.86","sz":"93.53","time":1681222254710,"fee":"0.01","feeToken":"USDC","builderFee":"0.01","tid":118906512037719}"#;

const TRADER: &str = "0xd80adf681abe30a3d644db1daae9abaa2d25a895";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn user() -> Address {
    TRADER.parse().unwrap()
}

#[test]
fn every_read_asks_in_the_venue_s_words() {
    assert_eq!(user_fills_request(user(), false), format!(r#"{{"type":"userFills","user":"{TRADER}"}}"#));
    assert_eq!(
        user_fills_request(user(), true),
        format!(r#"{{"type":"userFills","user":"{TRADER}","aggregateByTime":true}}"#)
    );
    assert_eq!(
        user_fills_by_time_request(user(), 1_791_480_420_000, Some(1_791_480_421_000), false),
        format!(r#"{{"type":"userFillsByTime","user":"{TRADER}","startTime":1791480420000,"endTime":1791480421000}}"#)
    );
    assert_eq!(
        user_fills_by_time_request(user(), 1_791_480_420_000, None, true),
        format!(r#"{{"type":"userFillsByTime","user":"{TRADER}","startTime":1791480420000,"aggregateByTime":true}}"#)
    );
    assert_eq!(user_twap_slice_fills_request(user()), format!(r#"{{"type":"userTwapSliceFills","user":"{TRADER}"}}"#));
    assert_eq!(user_fees_request(user()), format!(r#"{{"type":"userFees","user":"{TRADER}"}}"#));
}

#[test]
fn a_fill_reads_every_figure_exactly() {
    let fills: Vec<Fill> = serde_json::from_str(FILLS).unwrap();
    assert_eq!(fills.len(), 5);
    let close = &fills[0];
    assert_eq!((close.coin.as_str(), close.side, close.dir.as_str()), ("GRAM", Side::Sell, "Close Long"));
    assert_eq!((close.px, close.sz, close.closed_pnl), (dec("1.3415"), dec("423"), dec("-34.940646")));
    assert_eq!((close.fee, close.fee_token.as_str(), close.builder_fee), (dec("0.192934"), "USDC", None));
    assert_eq!(
        (close.oid, close.tid, close.crossed, close.twap_id),
        (569_016_338_230, 196_547_699_056_480, true, None)
    );
    let dust = &fills[2];
    assert_eq!((dust.dir.as_str(), dust.sz, dust.tid), ("Spot Dust Conversion", dec("0.00000001"), 0));
    let bought = &fills[4];
    assert_eq!((bought.fee_token.as_str(), bought.fee), ("UZEC", dec("0.000362456")), "a spot buy pays in the token");
}

#[test]
fn a_builder_fee_is_read_only_where_the_venue_writes_one() {
    let fill: Fill = serde_json::from_str(DOCS_FILL).unwrap();
    assert_eq!((fill.fee, fill.builder_fee), (dec("0.01"), Some(dec("0.01"))), "the fee includes the builder's");
    assert_eq!((fill.start_position, fill.cloid), (dec("26.86"), None));
}

#[test]
fn fills_by_time_split_and_aggregated_agree() {
    let split: Vec<Fill> = serde_json::from_str(BY_TIME).unwrap();
    let whole: Vec<Fill> = serde_json::from_str(AGGREGATED).unwrap();
    assert_eq!((split.len(), whole.len()), (14, 1));
    let total = split.iter().try_fold(Dec::ZERO, |sum, fill| sum.checked_add(fill.sz)).unwrap();
    assert_eq!(total, whole[0].sz);
    assert_eq!(whole[0].sz, dec("4095"));
    let fees = split.iter().try_fold(Dec::ZERO, |sum, fill| sum.checked_add(fill.fee)).unwrap();
    assert_eq!(fees, whole[0].fee, "1.868099 either way");
    assert_eq!(whole[0].px, dec("1.3417402442"), "an average past the grid, kept exactly");
}

#[test]
fn a_twap_slice_names_its_twap() {
    let slices: Vec<TwapSliceFill> = serde_json::from_str(SLICES).unwrap();
    assert_eq!(slices.len(), 3);
    assert_eq!((slices[0].twap_id, slices[0].fill.coin.as_str(), slices[0].fill.sz), (2_297_627, "NEAR", dec("2.6")));
    assert!(slices.iter().all(|slice| slice.fill.hash == format!("0x{}", "0".repeat(64))), "a slice has no hash");
}

#[test]
fn the_fee_schedule_reads_as_fractions() {
    let fees: UserFees = serde_json::from_str(FEES).unwrap();
    assert_eq!((fees.user_cross_rate, fees.user_add_rate), (dec("0.00034"), dec("0.000102")));
    assert_eq!((fees.user_spot_cross_rate, fees.user_spot_add_rate), (dec("0.00051"), dec("0.000255")));
    assert_eq!(fees.fee_schedule.cross, dec("0.00045"));
    assert_eq!(fees.fee_schedule.tiers.vip[0].ntl_cutoff, dec("5000000"));
    assert_eq!(fees.fee_schedule.tiers.mm[0].add, dec("-0.00001"), "a maker rebate is negative");
    assert_eq!(fees.fee_schedule.referral_discount, dec("0.04"));
    assert_eq!(fees.active_staking_discount.map(|tier| tier.discount), Some(dec("0.15")));
    assert_eq!(fees.daily_user_vlm[0].exchange, dec("9424822576.8999996185"));
    assert_eq!(fees.active_referral_discount, Dec::ZERO);
}

#[test]
fn each_read_goes_through_the_transport_once() {
    let transport = Recorded::answering(FILLS);
    assert_eq!(block_on(user_fills(&transport, user(), false)).unwrap().len(), 5);
    assert_eq!(transport.sent(), [("/info".to_owned(), user_fills_request(user(), false))]);
    let transport = Recorded::answering(BY_TIME);
    assert_eq!(block_on(user_fills_by_time(&transport, user(), 1, None, false)).unwrap().len(), 14);
    let transport = Recorded::answering(SLICES);
    assert_eq!(block_on(user_twap_slice_fills(&transport, user())).unwrap().len(), 3);
    let transport = Recorded::answering(FEES);
    assert!(block_on(user_fees(&transport, user())).is_ok());
}
