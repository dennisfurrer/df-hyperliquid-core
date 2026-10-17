//! The outcome fee model, measured on mainnet 2026-09-12 on the account whose `userFees` is below:
//! a maker fill billed `0.0004 * 2 * 0.96 = 0.000768` of notional and a taker
//! `0.0007 * 2 * 0.96 = 0.001344`, both exactly; every buy billed 0.

use super::*;
use crate::account::history::user_fees_request;
use crate::utils::Address;

/// A live `userFees` answer.
const FEES: &str = r#"{"dailyUserVlm":[{"date":"2026-09-25","userCross":"244273.31","userAdd":"977934.67","exchange":"7758060965.1000003815"},{"date":"2026-09-26","userCross":"22117.23","userAdd":"652639.39","exchange":"1205047792.5099999905"}],"userCrossRate":"0.00045","userAddRate":"0.00015","userSpotCrossRate":"0.0007","userSpotAddRate":"0.0004","activeReferralDiscount":"0.04","trial":null,"feeTrialEscrow":"0.0","nextTrialAvailableTimestamp":null,"stakingLink":null,"activeStakingDiscount":{"bpsOfMaxSupply":"0.0","discount":"0.0"}}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn fees() -> UserFees {
    serde_json::from_str(FEES).unwrap()
}

#[test]
fn outcomes_bill_off_the_spot_schedule_not_the_perp_one() {
    let fees = fees();
    assert_eq!(
        (fees.user_spot_cross_rate, fees.user_spot_add_rate, fees.active_referral_discount),
        (dec("0.0007"), dec("0.0004"), dec("0.04"))
    );
    assert_eq!(rate(&fees, false, Dec::ONE), Some(dec("0.000768")), "a maker fill, as measured");
    assert_eq!(rate(&fees, true, Dec::ONE), Some(dec("0.001344")), "a taker fill, as measured");
}

#[test]
fn the_deployer_s_scale_multiplies_by_itself_plus_at_least_one() {
    assert_eq!(multiplier(Dec::ONE), Some(dec("2")), "every live outcome on 2026-09-12");
    assert_eq!(multiplier(Dec::ZERO), Some(Dec::ONE));
    assert_eq!(multiplier(dec("0.5")), Some(dec("1.5")));
    assert_eq!(multiplier(dec("3")), Some(dec("6")));
    assert_eq!(deployer_share(Dec::ONE, Dec::ONE), Some(dec("0.5")));
    assert_eq!(deployer_share(Dec::ONE, Dec::ZERO), Some(Dec::ZERO));
    assert_eq!(deployer_share(Dec::ONE, dec("3")), Some(dec("0.5")));
    assert_eq!(deployer_share(Dec::ONE, dec("0.5")), Some(dec("0.33333333")), "to eight places, toward zero");
}

/// The deployer's part is multiplied out before the one division. A third cut to 0.33333333
/// first and then multiplied rounds twice: on the measured maker fee of 100 of notional
/// (0.0768) at scale 0.5 it gives 0.02559999, where Python's `decimal` says 0.0768 * 0.5 / 1.5
/// is 0.0256 exactly; on 1234.56789012 it gives 411.52262592 for 411.52263004.
#[test]
fn the_deployer_s_part_is_rounded_once_not_twice() {
    assert_eq!(deployer_share(dec("0.0768"), dec("0.5")), Some(dec("0.0256")));
    assert_eq!(deployer_share(dec("1234.56789012"), dec("0.5")), Some(dec("411.52263004")));
    assert_eq!(deployer_share(dec("7.25866"), dec("0.3")), Some(dec("1.67507538")), "1.675075384615..., cut");
}

#[test]
fn only_the_closing_side_pays() {
    let fees = fees();
    let notional = dec("7.25866");
    assert_eq!(billed(&fees, Side::Buy, true, Dec::ONE, notional), Some(Dec::ZERO), "every buy was billed 0");
    assert_eq!(billed(&fees, Side::Sell, true, Dec::ONE, notional), Some(dec("0.00975563904")));
    assert_eq!(billed(&fees, Side::Sell, false, Dec::ONE, dec("100")), Some(dec("0.0768")));
}

#[test]
fn a_fill_s_fee_is_estimated_from_what_it_was() {
    let fees = fees();
    let sell = Fill::read(
        &serde_json::from_str(
            r##"{"coin":"#12270","px":"0.5","sz":"200.0","side":"A","time":1,"startPosition":"200.0","dir":"Sell",
        "closedPnl":"0.0","hash":"0x","oid":1,"crossed":false,"fee":"0.0768","tid":1,"feeToken":"USDC"}"##,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(billed_for(&fees, &sell, Dec::ONE), Some(dec("0.0768")));
    assert_eq!(
        user_fees_request(Address::ZERO),
        r#"{"type":"userFees","user":"0x0000000000000000000000000000000000000000"}"#
    );
}
