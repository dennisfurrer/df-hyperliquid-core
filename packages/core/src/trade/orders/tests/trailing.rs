//! A trailing stop as the docs write it: a percent or a fixed retracement, an activation price or
//! `null`, and what the venue would refuse.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_trailing_stop_is_written_in_the_docs_order_with_its_retracement_and_activation() {
    let btc = TrailingStop::new(Asset::from_id(0), Side::Sell, dec("0.01"), Retracement::Percent(dec("1.5")))
        .unwrap()
        .reduce_only();
    assert_eq!(
        btc.wire(),
        r#"{"type":"trailingStop","asset":0,"isBuy":false,"sz":"0.01","reduceOnly":true,"retracement":{"pct":"1.5%"},"activationPx":null}"#
    );
    let tsla = TrailingStop::new(Asset::from_id(110_001), Side::Buy, dec("2"), Retracement::Price(dec("2.50")))
        .and_then(|stop| stop.activated_at(dec("410")))
        .unwrap();
    assert_eq!(
        tsla.wire(),
        r#"{"type":"trailingStop","asset":110001,"isBuy":true,"sz":"2","reduceOnly":false,"retracement":{"px":"2.5"},"activationPx":"410"}"#
    );
    assert_eq!(
        (tsla.activation(), tsla.is_reduce_only(), TrailingStop::KEY),
        (Some(dec("410")), false, Signers::Either)
    );
}

#[test]
fn a_size_retracement_or_activation_the_venue_would_refuse_is_refused_here() {
    let new = |size: &str, retracement| TrailingStop::new(Asset::from_id(0), Side::Sell, dec(size), retracement);
    assert_eq!(new("0", Retracement::Percent(dec("1"))), Err(BadTrailingStop::Size(dec("0"))));
    assert_eq!(new("1", Retracement::Percent(dec("100"))), Err(BadTrailingStop::Retracement(dec("100"))));
    assert_eq!(new("1", Retracement::Price(dec("-1"))), Err(BadTrailingStop::Retracement(dec("-1"))));
    assert!(new("1", Retracement::Price(dec("150"))).is_ok(), "a fixed amount has no cap");
    let stop = new("1", Retracement::Percent(dec("0.5"))).unwrap();
    assert_eq!(stop.activated_at(Dec::ZERO), Err(BadTrailingStop::Activation(Dec::ZERO)));
}
