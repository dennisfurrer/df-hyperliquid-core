//! `outcomeTemplates` as the venue answered it: mainnet on 2026-09-28 and testnet on 2026-10-08,
//! trimmed to eight templates.

use super::*;

const MAINNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));
const TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));

fn read(text: &str) -> Vec<OutcomeTemplate> {
    serde_json::from_str::<Templates>(text).unwrap().0
}

#[test]
fn versions_split_off_the_stem() {
    assert_eq!(Template::classify("template:binaryPrice4"), Template { family: Family::BinaryPrice, version: Some(4) });
    assert_eq!(
        Template::classify("template:sportsContestWinner"),
        Template { family: Family::ContestWinner, version: None }
    );
    assert_eq!(Template::classify("template:policyRateNoChange3").family, Family::PolicyRate(RateMove::NoChange));
}

#[test]
fn names_outside_the_templates_still_classify() {
    assert_eq!(Template::classify("template fallback").family, Family::Fallback);
    assert_eq!(Template::classify("Fallback").family, Family::Fallback, "testnet's spelling");
    assert_eq!(Template::classify("Recurring Fallback").family, Family::Fallback);
    assert_eq!(Template::classify("Recurring Named Outcome").family, Family::Recurring);
    assert_eq!(Template::classify("Argentina").family, Family::Named);
    assert_eq!(
        Template::classify("template:somethingNew").family,
        Family::Template,
        "a template this build has no family for is read from its pattern"
    );
}

#[test]
fn a_question_family_describes_a_question() {
    assert!(Family::ContestResult.is_question() && !Family::ContestWinner.is_question());
    assert!(Family::TournamentWinner.is_question() && Family::PolicyRateDecision.is_question());
}

const ROLES: &str = r#"[
    {"id":"binaryPrice","role":{"standaloneOutcome":{"sideNames":["Yes","No"]}},
     "name":"{perp} above {threshold} at {time}?","description":"...",
     "keywords":[["perp","hlPerp"],["threshold","uDecimal"],["time","dateTime"]]},
    {"id":"policyRateDecision","role":"question","name":"...","description":"...",
     "keywords":[["institution","string"]]},
    {"id":"policyRateIncrease","role":{"questionOutcome":{"parent":"policyRateDecision"}},
     "name":"...","description":"...","keywords":[]},
    {"id":"somethingNew","role":{"futureRole":{}},"name":"...","description":"...",
     "keywords":[]}]"#;

#[test]
fn a_role_this_build_has_never_seen_is_kept_by_name_rather_than_dropped() {
    let read = read(ROLES);
    assert_eq!(read[0].role, TemplateRole::Standalone { side_names: vec!["Yes".to_owned(), "No".to_owned()] });
    assert_eq!(read[1].role, TemplateRole::Question);
    assert_eq!(read[2].role, TemplateRole::QuestionOutcome { parent: "policyRateDecision".to_owned() });
    assert_eq!(read[3].role, TemplateRole::Other("futureRole".to_owned()));
    assert_eq!(read[0].keywords[0], Keyword { name: "perp".to_owned(), kind: "hlPerp".to_owned() });
    assert_eq!(read[0].keywords[0].hint(), Hint::HlPerp);
    assert_eq!(read[0].keywords.len(), 3);
}

#[test]
fn a_template_that_does_not_read_is_skipped_and_the_rest_kept() {
    let text = r#"[{"id":"binaryPrice","role":"question","name":"x","description":"",
        "keywords":[["perp"]]},{"id":"draw","role":"question","name":"Draw","description":"","keywords":[]}]"#;
    let read = read(text);
    assert_eq!(read.len(), 1, "a keyword with no kind is no template");
    assert_eq!(read[0].id, "draw");
}

#[test]
fn mainnet_s_templates_read_as_the_venue_wrote_them() {
    let templates = read(MAINNET);
    assert_eq!(templates.len(), 6);
    let binary = &templates[0];
    assert_eq!(binary.id, "binaryPrice");
    assert_eq!(binary.name, "{perp} above {threshold} at {time}?");
    assert_eq!(binary.role, TemplateRole::Standalone { side_names: vec!["Yes".to_owned(), "No".to_owned()] });
    let winner = templates.iter().find(|one| one.id == "sportsContestWinner").unwrap();
    assert_eq!(
        winner.role,
        TemplateRole::Standalone { side_names: vec!["{shortNameA}".to_owned(), "{shortNameB}".to_owned()] }
    );
    let named = templates.iter().filter(|one| one.parent() == Some("sportsTournamentWinner")).count();
    assert_eq!(named, 2, "the participant and the no-winner outcome");
    assert!(templates.iter().any(|one| one.id == "sportsTournamentWinner" && one.role == TemplateRole::Question));
}

#[test]
fn testnet_s_templates_carry_each_keyword_s_hint() {
    let templates = read(TESTNET);
    assert_eq!(templates.len(), 8);
    let binary = &templates[0];
    assert_eq!(binary.id, "binaryPrice4");
    let hints: Vec<(&str, Hint)> =
        binary.keywords.iter().map(|keyword| (keyword.name.as_str(), keyword.hint())).collect();
    assert_eq!(
        hints,
        [
            ("perp", Hint::HlPerp),
            ("priceDescription", Hint::String),
            ("seconds", Hint::UInt),
            ("threshold", Hint::UDecimal),
            ("time", Hint::DateTime),
        ]
    );
    let draw = templates.iter().find(|one| one.id == "sportsContestDraw").unwrap();
    assert_eq!(draw.parent(), Some("sportsContestResult"));
    assert!(draw.keywords.is_empty());
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

/// As outcome 1210 reads on mainnet.
#[test]
fn on_chain_a_row_is_its_template_and_its_keywords_sorted() {
    let templates = read(MAINNET);
    let binary = &templates[0];
    let values = Keywords::from_pairs([
        ("time", "20261001-0000"),
        ("perp", "BTC"),
        ("threshold", "100000"),
        ("seconds", "1"),
        ("priceDescription", "BTC-USDC mark"),
    ])
    .unwrap();
    let chain = binary.on_chain(&values);
    assert_eq!(chain.name, "template:binaryPrice");
    assert_eq!(
        chain.description,
        "perp:BTC|priceDescription:BTC-USDC mark|seconds:1|threshold:100000|time:20261001-0000"
    );
    assert_eq!(chain.sides, Some(["template:Yes".to_owned(), "template:No".to_owned()]));
    let named = templates.iter().find(|one| one.parent().is_some()).unwrap();
    assert_eq!(named.on_chain(&Keywords::new()).sides, None, "a question's outcome has no sides of its own");
}

#[test]
fn the_reads_name_their_type() {
    assert_eq!(outcome_templates_request(), r#"{"type":"outcomeTemplates"}"#);
    assert_eq!(deployer_limits_request("out"), r#"{"type":"outcomeDeployerLimits","venue":"out"}"#);
}

/// Live, 2026-09-13, on a venue holding 99.
#[test]
fn the_deployer_limits_read_what_remains() {
    let remaining: Remaining =
        serde_json::from_str(r#"{"nDailyOutcomesRemaining":494,"nActiveOutcomesRemaining":1}"#).unwrap();
    assert_eq!(remaining, Remaining { daily: 494, active: 1 });
    assert_eq!(serde_json::from_str::<Remaining>("{}").unwrap(), Remaining { daily: 0, active: 0 });
}
