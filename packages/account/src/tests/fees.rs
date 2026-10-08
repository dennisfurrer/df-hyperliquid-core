use super::*;
use crate::fixtures::{FEES_UNIFIED, Recorded, UNIFIED, address, block_on, dec, read};

/// hip4-tui's `net/src/tests/accounts.rs`, the live shape.
const FEES: &str = r#"{"dailyUserVlm":[
    {"date":"2026-09-17","userCross":"1498565.3600000001","userAdd":"0.0","exchange":"6184805063.68"},
    {"date":"2026-09-18","userCross":"0.0","userAdd":"250.5","exchange":"10088730521.48"},
    {"date":"2026-09-19","userCross":"12.25","userAdd":"0.0","exchange":"3362590973.46"}],
    "feeSchedule":{"cross":"0.00045","add":"0.00015","spotCross":"0.0007","spotAdd":"0.0004"},
    "userCrossRate":"0.00045","userAddRate":"0.00015","userSpotCrossRate":"0.0007",
    "userSpotAddRate":"0.0004","activeReferralDiscount":"0.0","trial":null,
    "nextTrialAvailableTimestamp":null,"stakingLink":null}"#;

#[test]
fn the_fee_answer_sums_the_days_it_sends_taker_and_maker_together() {
    let fees: Fees = read(FEES);
    assert_eq!(fees.volume(), Some(dec("1498828.1100000001")), "every day, exactly");
    assert_eq!(fees.daily_user_vlm[1].date, "2026-09-18");
    let none: Fees = read(r#"{"userCrossRate":"0.00045"}"#);
    assert_eq!(none.volume(), Some(Dec::ZERO), "no days is no volume");
}

/// A live account at a VIP tier: its own rates under the base schedule.
#[test]
fn a_live_account_s_rates_read_under_the_base_schedule() {
    let fees: Fees = read(FEES_UNIFIED);
    assert_eq!(
        (fees.user_cross_rate, fees.user_add_rate, fees.user_spot_cross_rate, fees.user_spot_add_rate),
        (dec("0.0003"), dec("0.00004"), dec("0.0004"), dec("0.0001"))
    );
    assert_eq!((bps(fees.user_cross_rate), bps(fees.user_add_rate)), (Some(dec("3")), Some(dec("0.4"))));
    let schedule = fees.fee_schedule.as_ref().unwrap();
    assert_eq!((schedule.cross, schedule.add), (dec("0.00045"), dec("0.00015")), "the base tier");
    assert_eq!(schedule.referral_discount, Some(dec("0.04")));
    assert_eq!(fees.active_staking_discount.map(|staking| staking.discount), Some(Dec::ZERO));
    assert_eq!(fees.daily_user_vlm.len(), 15, "fourteen days and today");
    assert_eq!(fees.volume(), Some(dec("131872229.1499999978")));
}

#[test]
fn fees_are_read_at_the_address() {
    let body = format!(r#"{{"type":"userFees","user":"{UNIFIED}"}}"#);
    assert_eq!(fees_request(address(UNIFIED)), body);
    let transport = Recorded::new(&[(&body, FEES_UNIFIED)]);
    assert_eq!(block_on(fees(&transport, address(UNIFIED))).unwrap().user_cross_rate, dec("0.0003"));
}
