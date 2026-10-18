//! Descriptions as the venue wrote them: outcome 1209 on mainnet, 2026-09-28, carries a value
//! ending in a space.

use super::*;

#[test]
fn a_description_reads_as_keyword_pairs_in_its_order_and_prose_as_none() {
    let fields =
        Fields::parse("perp:HYPE|priceDescription:HYPE-USDC perp mark |seconds:1|target:100|time:20261001-0000");
    let keys: Vec<&str> = fields.pairs().map(|(key, _)| key).collect();
    assert_eq!(keys, ["perp", "priceDescription", "seconds", "target", "time"]);
    assert_eq!(fields.get("priceDescription"), Some("HYPE-USDC perp mark "), "kept as written");
    assert_eq!(fields.text("priceDescription").as_deref(), Some("HYPE-USDC perp mark"), "trimmed as text");
    assert_eq!((fields.seconds("seconds"), fields.seconds("perp")), (Some(1), None));
    assert_eq!(fields.time("time").map(|at| at.to_string()), Some("2026-10-01 00:00Z".to_owned()));
    let colons = Fields::parse("perp:xyz:SKHX|threshold:1250|time:20260815-1126");
    assert_eq!(colons.get("perp"), Some("xyz:SKHX"), "a value may hold colons");
    assert_eq!(colons.time("time").map(|at| at.to_string()), Some("2026-08-15 11:26Z".to_owned()));
    let prose = "This outcome resolves to Yes if the game kicks off at 8:30 AM ET.";
    for nothing in [prose, "other", "", "a b:c|:d|e:"] {
        assert!(Fields::parse(nothing).is_empty(), "{nothing:?}");
    }
    assert_eq!(Fields::parse("participant:  |shortName:Cats").text("participant"), None, "blank is no text");
    assert_eq!(Fields::parse("participant:Cats").get("missing"), None);
}

#[test]
fn a_number_is_carried_across_exactly_as_the_chain_wrote_it() {
    let fields = Fields::parse("a:100000.00|b:-2.5|c:0.001|d:nine|e:1e8|f:.5|g:+3|h:-|list:76929, 80069,x");
    let cases = [
        ("a", Some("100000.00")),
        ("b", Some("-2.5")),
        ("c", Some("0.001")),
        ("f", Some(".5")),
        ("g", Some("+3")),
        ("d", None),
        ("e", None),
        ("h", None),
        ("missing", None),
    ];
    for (key, number) in cases {
        assert_eq!(fields.number(key).as_deref(), number, "{key}");
    }
    assert_eq!(fields.numbers("list"), ["76929", "80069"], "each as written, the rest dropped");
    assert!(fields.numbers("missing").is_empty());
}
