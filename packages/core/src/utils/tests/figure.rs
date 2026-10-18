//! Figures read from bare JSON floats and decimal strings, every digit kept.

use serde::Deserialize;

use super::*;

#[derive(Debug, Deserialize)]
struct Vault {
    apr: Figure,
    #[serde(default)]
    commission: Option<Figure>,
}

/// Live, mainnet HLP, 2026-10-06: eighteen places, which an f64 cannot carry and a `Dec` can.
/// An exponent is applied exactly; past eighteen places it is the nearest at eighteen.
#[test]
fn a_number_keeps_every_digit_it_was_written_with() {
    let vault: Vault = serde_json::from_str(r#"{"apr":0.037569521048262375,"commission":0.0}"#).unwrap();
    assert_eq!(vault.apr.as_str(), "0.037569521048262375");
    assert_eq!(vault.apr.to_string(), "0.037569521048262375");
    assert_eq!(vault.commission.and_then(|figure| figure.to_dec()), Some(Dec::ZERO));
    let none: Vault = serde_json::from_str(r#"{"apr":"0.05","commission":null}"#).unwrap();
    assert_eq!(none.apr.to_dec(), Dec::new(5, 2), "a decimal string reads too");
    assert_eq!(none.commission, None);

    for (text, value) in [
        ("0.037569521048262375", Dec::new(37_569_521_048_262_375, 18)),
        ("1e-7", Dec::new(1, 7)),
        ("2.5E3", Some(Dec::from_int(2500))),
        ("-4.2e-1", Dec::new(-42, 2)),
        ("1e-30", Some(Dec::ZERO)),
        ("41085245.621082", Dec::new(41_085_245_621_082, 6)),
    ] {
        assert_eq!(serde_json::from_str::<Figure>(text).unwrap().to_dec(), value, "{text}");
    }
}

#[test]
fn anything_but_a_number_is_refused() {
    for bad in ["true", "\"abc\"", "{}", "[1]", "null", "\"\""] {
        assert!(serde_json::from_str::<Figure>(bad).is_err(), "{bad}");
    }
}
