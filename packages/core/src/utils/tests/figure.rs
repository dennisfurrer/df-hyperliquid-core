//! Figures read from bare JSON floats and decimal strings, every digit kept.

use serde::Deserialize;

use super::*;

#[derive(Debug, Deserialize)]
struct Vault {
    apr: Figure,
    #[serde(default)]
    commission: Option<Figure>,
}

/// Live, mainnet HLP, 2026-10-06: eighteen places, which an f64 cannot
/// carry and a `Dec` can.
#[test]
fn a_float_keeps_every_digit_it_was_written_with() {
    let vault: Vault = serde_json::from_str(r#"{"apr":0.037569521048262375,"commission":0.0}"#).unwrap();
    assert_eq!(vault.apr.as_str(), "0.037569521048262375");
    assert_eq!(vault.apr.to_dec(), Dec::new(37_569_521_048_262_375, 18));
    assert_eq!(vault.commission.and_then(|figure| figure.to_dec()), Some(Dec::ZERO));
    let none: Vault = serde_json::from_str(r#"{"apr":"0.05","commission":null}"#).unwrap();
    assert_eq!(none.apr.to_dec(), Dec::new(5, 2), "a decimal string reads too");
    assert_eq!(none.commission, None);
}

#[test]
fn an_exponent_is_applied_exactly() {
    let read = |text: &str| serde_json::from_str::<Figure>(text).unwrap().to_dec();
    assert_eq!(read("1e-7"), Dec::new(1, 7));
    assert_eq!(read("2.5E3"), Some(Dec::from_int(2500)));
    assert_eq!(read("-4.2e-1"), Dec::new(-42, 2));
    assert_eq!(read("1e-30"), Some(Dec::ZERO), "past eighteen places is the nearest at eighteen");
    assert_eq!(read("41085245.621082"), Dec::new(41_085_245_621_082, 6));
}

#[test]
fn anything_but_a_number_is_refused() {
    for bad in ["true", "\"abc\"", "{}", "[1]", "null", "\"\""] {
        assert!(serde_json::from_str::<Figure>(bad).is_err(), "{bad}");
    }
}
