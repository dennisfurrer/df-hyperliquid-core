//! Descriptions as the venue wrote them.

use super::*;

#[test]
fn values_may_hold_colons() {
    let fields = Fields::parse("perp:xyz:SKHX|threshold:1250|time:20260815-1126");
    assert_eq!(fields.get("perp"), Some("xyz:SKHX"));
    assert_eq!(fields.number("threshold").as_deref(), Some("1250"));
    assert_eq!(fields.time("time").map(|at| at.to_string()), Some("2026-08-15 11:26Z".to_owned()));
}

#[test]
fn lists_split_on_commas() {
    let fields = Fields::parse("priceThresholds:76929,80069|period:1d");
    assert_eq!(fields.numbers("priceThresholds"), ["76929", "80069"]);
    assert!(fields.numbers("missing").is_empty());
}

#[test]
fn prose_parses_to_nothing() {
    let prose = "This outcome resolves to Yes if the game kicks off at 8:30 AM ET.";
    assert!(Fields::parse(prose).is_empty());
    assert!(Fields::parse("other").is_empty());
    assert_eq!(Fields::parse("participant:Cats").text("participant").as_deref(), Some("Cats"));
}

#[test]
fn a_number_is_carried_across_exactly_as_the_chain_wrote_it() {
    let fields = Fields::parse("a:100000.00|b:-2.5|c:0.001|d:nine|e:1e8|f:.5");
    assert_eq!(fields.number("a").as_deref(), Some("100000.00"));
    assert_eq!(fields.number("b").as_deref(), Some("-2.5"));
    assert_eq!(fields.number("c").as_deref(), Some("0.001"));
    assert_eq!(fields.number("f").as_deref(), Some(".5"));
    assert_eq!(fields.number("d"), None);
    assert_eq!(fields.number("e"), None);
    assert_eq!(fields.number("missing"), None);
}

/// A live description with a value ending in a space (outcome 1209, mainnet, 2026-09-28): the
/// space is kept by `get` and trimmed by `text`.
#[test]
fn a_value_keeps_its_spaces_until_it_is_read_as_text() {
    let fields =
        Fields::parse("perp:HYPE|priceDescription:HYPE-USDC perp mark |seconds:1|target:100|time:20261001-0000");
    assert_eq!(fields.get("priceDescription"), Some("HYPE-USDC perp mark "));
    assert_eq!(fields.text("priceDescription").as_deref(), Some("HYPE-USDC perp mark"));
    assert_eq!(fields.seconds("seconds"), Some(1));
    let keys: Vec<&str> = fields.pairs().map(|(key, _)| key).collect();
    assert_eq!(keys, ["perp", "priceDescription", "seconds", "target", "time"]);
}
