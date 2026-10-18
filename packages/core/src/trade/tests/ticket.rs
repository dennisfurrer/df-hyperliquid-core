//! A ticket's market is only the kind its asset id names.

use super::*;
use crate::utils::Asset;

#[test]
fn a_market_is_only_the_kind_its_asset_id_names() {
    assert!(Market::perp(Asset::from_id(10_107), 2, 3, true).is_none(), "a spot id is not a perp");
    assert!(Market::spot(Asset::from_id(0), 2).is_none(), "a perp id is not a pair");
    assert!(Market::perp(Asset::from_id(100_012_100), 0, 1, true).is_none(), "an outcome is an outcome market's");
    assert!(Market::spot(Asset::from_id(100_012_100), 0).is_none());
    let tsla = Market::perp(Asset::from_id(110_001), 3, 10, false).unwrap();
    assert_eq!((tsla.max_leverage(), tsla.allows_cross(), tsla.is_spot()), (Some(10), false, false));
    let hype = Market::spot(Asset::from_id(10_107), 2).unwrap();
    assert_eq!((hype.max_leverage(), hype.is_spot()), (None, true));
}
