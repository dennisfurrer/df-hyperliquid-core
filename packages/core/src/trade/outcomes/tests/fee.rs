//! The outcome fee model, measured on mainnet 2026-09-12 on the account whose `userFees` is below:
//! a maker fill billed `0.0004 * 2 * 0.96 = 0.000768` of notional and a taker
//! `0.0007 * 2 * 0.96 = 0.001344`, both exactly; every buy billed 0.

use super::*;

/// A live `userFees` answer.
const FEES: &str = r#"{"dailyUserVlm":[{"date":"2026-09-25","userCross":"244273.31","userAdd":"977934.67","exchange":"7758060965.1000003815"},{"date":"2026-09-26","userCross":"22117.23","userAdd":"652639.39","exchange":"1205047792.5099999905"}],"userCrossRate":"0.00045","userAddRate":"0.00015","userSpotCrossRate":"0.0007","userSpotAddRate":"0.0004","activeReferralDiscount":"0.04","trial":null,"feeTrialEscrow":"0.0","nextTrialAvailableTimestamp":null,"stakingLink":null,"activeStakingDiscount":{"bpsOfMaxSupply":"0.0","discount":"0.0"}}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn an_outcome_fill_is_billed_off_the_spot_schedule_on_the_selling_side_only() {
    let fees: UserFees = serde_json::from_str(FEES).unwrap();
    assert_eq!(rate(&fees, false, Dec::ONE), Some(dec("0.000768")), "a maker fill, as measured");
    assert_eq!(rate(&fees, true, Dec::ONE), Some(dec("0.001344")), "a taker fill, as measured");
    for (side, crossed, notional, billed_at) in [
        (Side::Buy, true, "7.25866", "0"),
        (Side::Sell, true, "7.25866", "0.00975563904"),
        (Side::Sell, false, "100", "0.0768"),
    ] {
        assert_eq!(billed(&fees, side, crossed, Dec::ONE, dec(notional)), Some(dec(billed_at)), "{side:?} {notional}");
    }
    let sell = Fill::read(
        &serde_json::from_str(
            r##"{"coin":"#12270","px":"0.5","sz":"200.0","side":"A","time":1,"startPosition":"200.0","dir":"Sell",
        "closedPnl":"0.0","hash":"0x","oid":1,"crossed":false,"fee":"0.0768","tid":1,"feeToken":"USDC"}"##,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(billed_for(&fees, &sell, Dec::ONE), Some(dec("0.0768")), "what the venue billed it");
}

/// The deployer's part is multiplied out before the one division. A third cut to 0.33333333
/// first and then multiplied rounds twice: on the measured maker fee of 100 of notional
/// (0.0768) at scale 0.5 it gives 0.02559999, where Python's `decimal` says 0.0768 * 0.5 / 1.5
/// is 0.0256 exactly; on 1234.56789012 it gives 411.52262592 for 411.52263004.
#[test]
fn the_deployer_s_part_is_its_scale_over_the_multiplier_rounded_once() {
    for (scale, multiplied) in [("1", "2"), ("0", "1"), ("0.5", "1.5"), ("3", "6")] {
        assert_eq!(multiplier(dec(scale)), Some(dec(multiplied)), "scale {scale}");
    }
    for (fee, scale, share) in [
        ("1", "1", "0.5"),
        ("1", "0", "0"),
        ("1", "3", "0.5"),
        ("1", "0.5", "0.33333333"),
        ("0.0768", "0.5", "0.0256"),
        ("1234.56789012", "0.5", "411.52263004"),
        ("7.25866", "0.3", "1.67507538"),
    ] {
        assert_eq!(deployer_share(dec(fee), dec(scale)), Some(dec(share)), "{fee} at scale {scale}");
    }
}
