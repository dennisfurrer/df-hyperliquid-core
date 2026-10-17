use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::Asset;

#[test]
fn prices_and_sizes_never_pass_through_a_float() {
    // 0.1 + 0.2 is 0.3 exactly, and so is what is signed
    assert_eq!(dec("0.1").checked_add(dec("0.2")), Some(dec("0.3")));
    assert!(
        wire(order(&eth(), &limit(Side::Buy, "0.3", "33.3334", Tif::Gtc), None)).contains(r#""p":"0.3","s":"33.3334""#)
    );
}

#[test]
fn a_market_is_only_the_kind_its_asset_id_names() {
    assert!(Market::perp(Asset::from_id(10_107), 2, 3, true).is_none(), "a spot id is not a perp");
    assert!(Market::spot(Asset::from_id(0), 2).is_none(), "a perp id is not a pair");
    assert!(Market::perp(Asset::from_id(100_012_100), 0, 1, true).is_none(), "an outcome is an outcome market's");
    assert!(Market::perp(Asset::from_id(110_001), 3, 10, false).is_some_and(|market| !market.allows_cross()));
}
