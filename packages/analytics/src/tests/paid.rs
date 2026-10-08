//! hip4-tui's `tui/src/tests/paid.rs`, its fills written as the venue sends them; hl-hip4's fee
//! test, which carries the maker fill measured on mainnet (D11: `0.0004 * 2 * 0.96 = 0.000768`
//! of notional); and a venue split worked out by hand.

use hl_core::Dec;
use hl_hip4::{Catalog, Fill, OutcomeMeta, RawFill};

use super::*;
use crate::Basis;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn fill(coin: &str, px: &str, sz: &str, fee: &str, builder: Option<&str>, crossed: bool) -> Fill {
    let builder = builder.map_or_else(String::new, |paid| format!(r#","builderFee":"{paid}""#));
    let raw: RawFill = serde_json::from_str(&format!(
        r#"{{"coin":"{coin}","px":"{px}","sz":"{sz}","side":"A","time":1,"startPosition":"{sz}","dir":"Sell",
        "closedPnl":"0.0","hash":"0x00","oid":1,"crossed":{crossed},"fee":"{fee}","tid":1,"feeToken":"USDC"{builder}}}"#
    ))
    .unwrap();
    Fill::read(&raw).unwrap()
}

fn sell(px: &str, sz: &str, fee: &str, builder: Option<&str>, crossed: bool) -> Fill {
    fill("#12100", px, sz, fee, builder, crossed)
}

#[test]
fn a_whole_token_of_notional_is_price_times_contracts() {
    assert_eq!(sell("0.5", "2", "0", None, true).notional(), Dec::ONE);
}

#[test]
fn the_rate_is_measured_on_what_was_charged_and_not_on_all_of_it() {
    let charged = sell("0.5", "200", "0.1344", None, true);
    let opened = sell("0.5", "200", "0", None, true);
    let paid = Paid::of([&charged, &opened]);
    assert_eq!(paid.fills, 2);
    assert_eq!(paid.notional, dec("200"));
    assert_eq!(paid.charged, dec("100"));
    assert_eq!(paid.rate(), Some(1_344), "13.44 bp of what paid");
    assert_eq!(paid.blended(), Some(672), "half that over all of it");
}

#[test]
fn a_builder_fee_comes_out_of_the_fee_and_not_beside_it() {
    let paid = Paid::of([&sell("0.5", "200", "0.1344", Some("0.0344"), true)]);
    assert_eq!(paid.fee, dec("0.1344"));
    assert_eq!(paid.builder, dec("0.0344"));
    assert_eq!(paid.venue(), dec("0.1"), "what the venue kept");
    let deployer = |scale: &str| paid.deployer(dec(scale)).map(|estimate| estimate.amount);
    assert_eq!(deployer("1"), Some(dec("0.05")));
    assert_eq!(deployer("2"), Some(dec("0.05")));
    assert_eq!(deployer("0.5"), Some(dec("0.03333333")), "to eight places, toward zero");
    assert_eq!(deployer("0"), Some(Dec::ZERO));
    assert_eq!(paid.deployer(Dec::ONE).map(|estimate| estimate.basis), Some(Basis::Fills { fills: 1 }));
}

#[test]
fn nothing_traded_is_no_rate_rather_than_a_rate_of_nothing() {
    let paid = Paid::default();
    assert_eq!(paid.rate(), None);
    assert_eq!(paid.blended(), None);
    assert_eq!(paid.deployer(Dec::ONE).map(|estimate| estimate.amount), Some(Dec::ZERO));
    assert!(paid.all_taken(), "and no fill is every fill");
}

#[test]
fn a_maker_is_counted_apart_from_a_taker() {
    let took = sell("0.5", "200", "0.00001", None, true);
    let rested = sell("0.5", "200", "0.00001", None, false);
    let paid = Paid::of([&took, &rested]);
    assert_eq!((paid.fills, paid.taken), (2, 1));
    assert!(!paid.all_taken());
    assert_eq!(paid.fee, dec("0.00002"));
}

#[test]
fn the_maker_fill_measured_on_mainnet_pays_its_deployer_half_of_what_the_venue_kept() {
    let paid = Paid::of([&fill("#12270", "0.5", "200.0", "0.0768", None, false)]);
    assert_eq!(paid.rate(), Some(768), "0.0004 * 2 * 0.96");
    assert_eq!(paid.deployer(Dec::ONE).map(|estimate| estimate.amount), Some(dec("0.0384")));
}

#[test]
fn fills_are_split_by_the_venue_that_listed_their_market_at_its_own_scale() {
    let meta: OutcomeMeta = serde_json::from_str(
        r#"{"outcomes":[
          {"outcome":1209,"name":"n","description":"","sideSpecs":[{"name":"Yes"},{"name":"No"}],"venue":"out","deployerFeeScale":"1.0"},
          {"outcome":1210,"name":"n","description":"","sideSpecs":[{"name":"Yes"},{"name":"No"}],"venue":"out","deployerFeeScale":"0.5"},
          {"outcome":2494,"name":"n","description":"","sideSpecs":[{"name":"Yes"},{"name":"No"}],"venue":"txyz"},
          {"outcome":3719,"name":"n","description":"","sideSpecs":[{"name":"Yes"},{"name":"No"}],"deployerFeeScale":"1.0"}],
          "questions":[],"deployers":[]}"#,
    )
    .unwrap();
    let catalog = Catalog::build(&meta, 0);
    let fills = [
        fill("#12090", "0.5", "200", "0.1344", Some("0.0344"), true),
        fill("#12091", "0.5", "200", "0.1", None, true),
        fill("#12101", "0.4", "100", "0.06", None, false),
        fill("#24940", "0.5", "100", "0.05", None, true),
        fill("#37190", "0.5", "100", "0.05", None, true),
        fill("#99990", "0.5", "100", "0.05", None, true),
    ];
    let takes = by_venue(&fills, &catalog, None);
    let out = takes.venues.iter().find(|take| take.venue == Some("out")).unwrap();
    assert_eq!((out.paid.fills, out.paid.venue()), (3, dec("0.26")));
    assert_eq!(
        out.deployer.map(|estimate| estimate.amount),
        Some(dec("0.12")),
        "half of the 0.2 kept at a scale of one, a third of the 0.06 at a half"
    );
    assert_eq!(out.unscaled, 0);
    let txyz = takes.venues.iter().find(|take| take.venue == Some("txyz")).unwrap();
    assert_eq!((txyz.unscaled, txyz.deployer.map(|estimate| estimate.amount)), (1, Some(Dec::ZERO)), "no scale listed");
    let own = takes.venues.iter().find(|take| take.venue.is_none()).unwrap();
    assert_eq!((own.paid.fills, own.deployer), (1, None), "the venue's own markets have no deployer to pay");
    assert_eq!(takes.unlisted.fills, 1, "a market the catalogue no longer lists");

    let assumed = by_venue(&fills, &catalog, Some(Dec::ONE));
    let txyz = assumed.venues.iter().find(|take| take.venue == Some("txyz")).unwrap();
    assert_eq!((txyz.unscaled, txyz.deployer.map(|estimate| estimate.amount)), (0, Some(dec("0.025"))));
}
