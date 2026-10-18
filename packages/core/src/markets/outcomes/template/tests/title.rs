//! A title and side names filled from a row's keywords, on testnet's templates as the venue answered
//! them on 2026-10-08.

use super::*;
use crate::markets::outcomes::template::listing::{OutcomeTemplate, Templates};

const TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));

fn read(text: &str) -> Vec<OutcomeTemplate> {
    serde_json::from_str::<Templates>(text).unwrap().0
}

/// Testnet's `sportsSpread` title filled from a row's keywords, and a side's name from its spec.
#[test]
fn a_title_and_side_names_are_filled_from_the_keywords() {
    let templates = read(TESTNET);
    let spread = templates.iter().find(|one| one.id == "sportsSpread").unwrap();
    let fields = Fields::parse(
        "countedPlay:regulation time and any overtime|measure:points|participantA:Pittsburgh Steelers|participantB:Cleveland Browns|shortNameA:Steelers|spread:-2.5",
    );
    assert_eq!(fill(&spread.name, |name| fields.text(name)), "Pittsburgh Steelers v Cleveland Browns: Steelers -2.5");
    assert_eq!(fill("{company} IPO confirmed by {dateTime}", |name| fields.text(name)), "? IPO confirmed by ?");
    assert_eq!(fill("no keywords", |_| None), "no keywords");
    assert_eq!(fill("open {brace", |_| None), "open {brace", "an unclosed brace is kept as written");
    let game = Fields::parse("shortNameA:49ers|shortNameB:Rams");
    assert_eq!(label(Some("template:{shortNameA}"), &game, "Yes"), "49ers");
    assert_eq!(label(Some("template:{shortNameC}"), &game, "No"), "No", "a keyword the row leaves out");
    assert_eq!(label(Some("template:Yes"), &game, "No"), "Yes");
    assert_eq!(label(None, &game, "Yes"), "Yes");
}
