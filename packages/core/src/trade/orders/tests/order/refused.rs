//! What the venue would refuse, known before signing: a builder fee over its market's cap, and a TWAP,
//! leverage or margin the venue does not take.

use crate::trade::builder::fee::BuilderFee;
use crate::trade::margin::{MarginMode, UpdateIsolatedMargin, UpdateLeverage};
use crate::trade::orders::*;
use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::{address, asset, dec, limit};

/// The caps: 0.1% on a perp, main dex or HIP-3, 1% on spot and on an outcome; one perp order holds
/// the action to the perp cap.
#[test]
fn a_builder_takes_at_most_a_tenth_of_a_percent_on_perps_and_one_percent_elsewhere() {
    let paying = |orders: Vec<Order>, fee: u64| {
        PlaceOrder::new(orders)
            .through(Builder::new(address("0x1111111111111111111111111111111111111111"), BuilderFee::from_tenths(fee)))
    };
    let perp = limit(0, Side::Buy, "100", "1", Tif::Gtc);
    let hip3 = limit(110_001, Side::Buy, "100", "1", Tif::Gtc);
    let spot = limit(10_107, Side::Buy, "44", "1", Tif::Gtc);
    let outcome = limit(100_012_100, Side::Buy, "0.5", "20", Tif::Gtc);
    let refused = |action: PlaceOrder| action.over_builder_cap().map(|over| over.to_string());
    assert_eq!(refused(paying(vec![perp], 100)), None);
    assert_eq!(
        refused(paying(vec![perp], 101)).as_deref(),
        Some("the builder fee of 101 is over the 100 a perp order allows (0.1%)")
    );
    assert_eq!(
        refused(paying(vec![hip3], 101)).as_deref(),
        Some("the builder fee of 101 is over the 100 a HIP-3 perp order allows (0.1%)")
    );
    assert_eq!(refused(paying(vec![spot], 1000)), None);
    assert_eq!(
        refused(paying(vec![spot], 1001)).as_deref(),
        Some("the builder fee of 1001 is over the 1000 a spot order allows (1%)")
    );
    assert_eq!(refused(paying(vec![outcome], 1000)), None, "an outcome takes the spot cap");
    assert_eq!(
        refused(paying(vec![outcome], 1001)).as_deref(),
        Some("the builder fee of 1001 is over the 1000 an outcome order allows (1%)")
    );
    assert!(refused(paying(vec![spot, perp], 500)).is_some(), "one perp order holds the action to the perp cap");
    assert_eq!(PlaceOrder::one(perp).over_builder_cap(), None, "no builder, no cap");
}

#[test]
fn what_would_be_refused_is_refused_before_signing() {
    use crate::trade::error::TradeError;

    assert_eq!(Twap::new(asset(0), Side::Buy, dec("1"), 4), Err(TradeError::TwapMinutes(4)));
    assert_eq!(Twap::new(asset(0), Side::Buy, dec("1"), 1441), Err(TradeError::TwapMinutes(1441)));
    assert!(
        Twap::new(asset(0), Side::Buy, dec("1"), 5).is_ok() && Twap::new(asset(0), Side::Buy, dec("1"), 1440).is_ok()
    );
    assert_eq!(UpdateLeverage::new(asset(0), MarginMode::Cross, 0), Err(TradeError::NoLeverage));
    assert!(UpdateLeverage::new(asset(0), MarginMode::Cross, 1).unwrap().wire().contains(r#""isCross":true"#));
    assert!(matches!(
        UpdateIsolatedMargin::new(asset(1), Side::Buy, dec("1.0000001")),
        Err(TradeError::Margin(crate::utils::BadUnits::TooFine { places: 6, .. }))
    ));
    assert!(matches!(UpdateIsolatedMargin::new(asset(1), Side::Sell, Dec::ZERO), Err(TradeError::Margin(_))));
    assert_eq!(UpdateIsolatedMargin::new(asset(1), Side::Sell, dec("-0.000001")).unwrap().ntli(), -1);
}
