//! Live answers, mainnet, 2026-10-08, from one account: fills of each direction and fee token, one
//! market sell's fills by time and aggregated, three TWAP slices and its fee schedule. The builder
//! fill is the venue's own example from its info endpoint docs, since no recorded fill paid one.
//! Beside them a recorded socket `userFills` row from a liquidation, and two more `userFees`
//! answers, one trimmed to what a fee figure needs.

use super::*;
use crate::account::fixtures::{FEES_UNIFIED, UNIFIED, address};
use crate::trade::tests_support::{Recorded, block_on};

const FILLS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills.json"));
const BY_TIME: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills_by_time.json"));
const AGGREGATED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills_by_time_aggregated.json"));
const SLICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_twap_slice_fills.json"));
const FEES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fees.json"));

/// The info endpoint page's perp fill, comments taken out.
const DOCS_FILL: &str = r#"{"closedPnl":"0.0","coin":"AVAX","crossed":false,"dir":"Open Long","hash":"0xa166e3fa63c25663024b03f2e0da011a00307e4017465df020210d3d432e7cb8","oid":90542681,"px":"18.435","side":"B","startPosition":"26.86","sz":"93.53","time":1681222254710,"fee":"0.01","feeToken":"USDC","builderFee":"0.01","tid":118906512037719}"#;

const LIQUIDATION: &str = r#"{"coin":"BTC","px":"80432.0","sz":"0.02603","side":"B","time":1791480388855,"startPosition":"-12.56343","dir":"Close Short","closedPnl":"16.680024","hash":"0xeba0ed56a1ceefc8ed1a04462006440000dc053c3cc20e9a8f6998a960c2c9b3","oid":569015257022,"crossed":false,"fee":"-0.062809","tid":1051492541339376,"cloid":"0xf82e29e34219a1dcc66c6960c49a6f00","liquidation":{"liquidatedUser":"0xf467f1f46cad3f1041c765765be2891245144f6e","markPx":"80429.0","method":"market"},"feeToken":"USDC","twapId":null}"#;
/// The live `userFees` shape, its schedule trimmed to the base tier.
const FEES_TRIMMED: &str = r#"{"dailyUserVlm":[
    {"date":"2026-09-17","userCross":"1498565.3600000001","userAdd":"0.0","exchange":"6184805063.68"},
    {"date":"2026-09-18","userCross":"0.0","userAdd":"250.5","exchange":"10088730521.48"},
    {"date":"2026-09-19","userCross":"12.25","userAdd":"0.0","exchange":"3362590973.46"}],
    "feeSchedule":{"cross":"0.00045","add":"0.00015","spotCross":"0.0007","spotAdd":"0.0004"},
    "userCrossRate":"0.00045","userAddRate":"0.00015","userSpotCrossRate":"0.0007",
    "userSpotAddRate":"0.0004","activeReferralDiscount":"0.0","trial":null,
    "nextTrialAvailableTimestamp":null,"stakingLink":null}"#;

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

#[test]
fn a_fill_keeps_what_only_some_fills_carry_as_options() {
    let read: Fill = serde_json::from_str(DOCS_FILL).unwrap();
    assert_eq!((read.builder_fee, read.cloid.as_deref(), read.liquidation.as_ref()), (Some(dec("0.01")), None, None));
    let liquidated: Fill = serde_json::from_str(LIQUIDATION).unwrap();
    assert_eq!((liquidated.builder_fee, liquidated.twap_id), (None, None));
    assert_eq!(liquidated.cloid.as_deref(), Some("0xf82e29e34219a1dcc66c6960c49a6f00"));
    assert_eq!(
        liquidated.liquidation,
        Some(FillLiquidation {
            liquidated_user: Some("0xf467f1f46cad3f1041c765765be2891245144f6e".parse().unwrap()),
            mark_px: dec("80429"),
            method: "market".to_owned(),
        })
    );
}

#[test]
fn a_fill_without_its_fee_token_or_whether_it_crossed_is_refused() {
    for gone in ["\"feeToken\":\"USDC\",", "\"crossed\":false,"] {
        let text = LIQUIDATION.replace(gone, "");
        assert!(serde_json::from_str::<Fill>(&text).is_err(), "without {gone}");
    }
}

#[test]
fn the_fee_answer_sums_the_days_it_sends_taker_and_maker_together() {
    let fees: UserFees = serde_json::from_str(FEES_TRIMMED).unwrap();
    assert_eq!(fees.volume(), Some(dec("1498828.1100000001")), "every day, exactly");
    assert_eq!(fees.daily_user_vlm[1].date, "2026-09-18");
    let none: UserFees = serde_json::from_str(r#"{"userCrossRate":"0.00045"}"#).unwrap();
    assert_eq!(none.volume(), Some(Dec::ZERO), "no days is no volume");
}

/// A live account at a VIP tier: its own rates under the base schedule.
#[test]
fn a_live_account_s_rates_read_under_the_base_schedule() {
    let fees: UserFees = serde_json::from_str(FEES_UNIFIED).unwrap();
    assert_eq!(
        (fees.user_cross_rate, fees.user_add_rate, fees.user_spot_cross_rate, fees.user_spot_add_rate),
        (dec("0.0003"), dec("0.00004"), dec("0.0004"), dec("0.0001"))
    );
    let schedule = &fees.fee_schedule;
    assert_eq!((schedule.cross, schedule.add), (dec("0.00045"), dec("0.00015")), "the base tier");
    assert_eq!(schedule.referral_discount, dec("0.04"));
    assert_eq!(fees.active_staking_discount.map(|staking| staking.discount), Some(Dec::ZERO));
    assert_eq!(fees.daily_user_vlm.len(), 15, "fourteen days and today");
    assert_eq!(fees.volume(), Some(dec("131872229.1499999978")));
}

#[test]
fn fees_are_read_at_the_address() {
    let body = format!(r#"{{"type":"userFees","user":"{UNIFIED}"}}"#);
    assert_eq!(user_fees_request(address(UNIFIED)), body);
    let transport = Recorded::answering(FEES_UNIFIED);
    assert_eq!(block_on(user_fees(&transport, address(UNIFIED))).unwrap().user_cross_rate, dec("0.0003"));
    assert_eq!(transport.sent(), [("/info".to_owned(), body)]);
}
