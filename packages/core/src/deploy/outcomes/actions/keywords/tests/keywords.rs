//! Keywords as a caller gives them: the order they are hashed in, the rules every value keeps, and a
//! whole instance checked against what its template declares.

use super::*;
use crate::deploy::outcomes::actions::support::{NOW_MS, keywords};
use crate::markets::outcomes::template::Templates;

const TESTNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));
const MAINNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));

#[test]
fn keywords_come_out_sorted_bytewise_however_they_go_in() {
    let pairs = [
        ("time", "1"),
        ("zeta", "2"),
        ("\u{e9}clair", "3"),
        ("participantB", "4"),
        ("Zone", "5"),
        ("participantA", "6"),
        ("_x", "7"),
        ("apple", "8"),
        ("Apple", "9"),
    ];
    let sorted = concat!(
        r#"[["Apple","9"],["Zone","5"],["_x","7"],["apple","8"],["participantA","6"],"#,
        r#"["participantB","4"],["time","1"],["zeta","2"],[""#,
        "\u{e9}",
        r#"clair","3"]]"#,
    );
    let forward = keywords(&pairs);
    let mut reversed = pairs;
    reversed.reverse();
    let backward = keywords(&reversed);
    for built in [&forward, &backward] {
        assert_eq!(built.wire().to_json().compact(), sorted, "upper case before lower case, ASCII before the rest");
    }
    assert_eq!(forward, backward);
    assert_eq!(forward.len(), 9);
    assert_eq!(forward.get("Zone"), Some("5"));
    assert_eq!(forward.get("zone"), None);
    assert_eq!(Keywords::new().wire().to_json().compact(), "[]", "a template with no keywords");
    assert!(Keywords::new().is_empty());
}

#[test]
fn a_keyword_takes_one_plain_value() {
    let refused = |pairs: &[(&str, &str)]| Keywords::from_pairs(pairs.iter().copied()).unwrap_err().to_string();
    assert_eq!(refused(&[("participant", "Brazil"), ("participant", "Spain")]), "participant is given twice");
    assert_eq!(refused(&[("perp", "")]), "perp needs a value");
    assert_eq!(refused(&[("perp", "BTC|ETH")]), "perp cannot hold {, } or |");
    assert_eq!(refused(&[("perp", "{perp}")]), "perp cannot hold {, } or |");
    let too_long = "x".repeat(101);
    let too_many_bytes = "\u{e9}".repeat(51);
    assert_eq!(refused(&[("priceDescription", too_long.as_str())]), "priceDescription takes at most 100 characters");
    assert_eq!(
        refused(&[("priceDescription", too_many_bytes.as_str())]),
        "priceDescription takes at most 100 characters",
        "counted in bytes: 51 of them are 102"
    );
    let longest = "x".repeat(100);
    assert_eq!(keywords(&[("priceDescription", longest.as_str())]).get("priceDescription"), Some(longest.as_str()));
}

/// An instance is checked whole against what its template declares: a missing value, one the
/// template does not declare, and each its hint refuses, in that order. Against the venue's own
/// templates (2026-10-08): mainnet's `binaryPrice` takes the recorded preset whole; testnet's
/// `sportsContestResult` asks for eight more keywords than the Python SDK's two.
#[test]
fn a_template_instance_is_checked_whole_against_what_its_template_declares() {
    let declared = || {
        [
            ("perp", Hint::parse("hlPerp")),
            ("priceDescription", Hint::parse("string")),
            ("seconds", Hint::parse("uInt")),
            ("threshold", Hint::parse("uDecimal")),
            ("time", Hint::parse("dateTime")),
        ]
    };
    let bad = keywords(&[
        ("perp", "BTC"),
        ("seconds", "060"),
        ("threshold", "83114"),
        ("time", "20271001-1201"),
        ("colour", "red"),
    ]);
    let problems: Vec<String> = bad.check(declared(), NOW_MS).iter().map(ToString::to_string).collect();
    assert_eq!(
        problems,
        [
            "priceDescription needs a value",
            "seconds takes a whole number with no sign or leading zeros",
            "time is more than a year away",
            "colour is not a keyword of this template",
        ]
    );
    assert_eq!(Keywords::new().check([], NOW_MS), [], "a template with no keywords takes none");

    let declared = |templates: &[crate::markets::outcomes::OutcomeTemplate], id: &str| -> Vec<(String, Hint)> {
        let template = templates.iter().find(|one| one.id == id).unwrap();
        template.keywords.iter().map(|keyword| (keyword.name.clone(), keyword.hint())).collect()
    };
    let testnet = serde_json::from_str::<Templates>(TESTNET_TEMPLATES).unwrap().0;
    let result = declared(&testnet, "sportsContestResult");
    let typed = keywords(&[("participantB", "Spain"), ("participantA", "Brazil")]);
    let missing: Vec<String> = typed
        .check(result.iter().map(|(name, hint)| (name.as_str(), hint.clone())), NOW_MS)
        .into_iter()
        .filter(|bad| bad.problem == Problem::Missing)
        .map(|bad| bad.keyword)
        .collect();
    assert_eq!(
        missing,
        [
            "competition",
            "contestType",
            "officialSource",
            "resolutionDeadline",
            "scheduledStart",
            "season",
            "sport",
            "stage"
        ]
    );
    let mainnet = serde_json::from_str::<Templates>(MAINNET_TEMPLATES).unwrap().0;
    let binary = declared(&mainnet, "binaryPrice");
    let preset = keywords(&[
        ("threshold", "83114"),
        ("time", "20261231-1200"),
        ("seconds", "60"),
        ("priceDescription", "BTC-USDC perp mark"),
        ("perp", "BTC"),
    ]);
    assert_eq!(preset.check(binary.iter().map(|(name, hint)| (name.as_str(), hint.clone())), NOW_MS), []);
}
