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
fn kinds_round_trip_through_their_slugs() {
    for family in Family::ALL {
        assert_eq!(family.slug().parse::<Family>(), Ok(family));
        assert_eq!(family.to_string(), family.slug());
    }
    assert!("no_such_kind".parse::<Family>().is_err());
    assert!(Family::ContestResult.is_question() && !Family::ContestWinner.is_question());
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

#[test]
fn a_pattern_is_a_template_as_the_catalogue_reads_it() {
    let patterns = patterns(&read(TESTNET));
    let spread = &patterns["sportsSpread"];
    assert_eq!(spread.name, "{participantA} v {participantB}: {shortNameA} {spread}");
    assert_eq!(spread.sides, ["Yes", "No"]);
    assert_eq!(spread.keywords[7], ("shortNameA".to_owned(), "shortString".to_owned()));
    assert!(patterns["awardWinner"].sides.is_empty(), "a question has no sides of its own");
}

#[test]
fn the_reads_name_their_type() {
    assert_eq!(outcome_templates_request(), r#"{"type":"outcomeTemplates"}"#);
    assert_eq!(deployer_limits_request("out"), r#"{"type":"outcomeDeployerLimits","venue":"out"}"#);
}

/// Live, 2026-09-13: `{"nDailyOutcomesRemaining":494,"nActiveOutcomesRemaining":1}` on a venue
/// holding 99, and 73 and 12 held on two others.
#[test]
fn the_active_cap_is_derived_from_the_live_count_and_not_from_a_constant() {
    let remaining: Remaining =
        serde_json::from_str(r#"{"nDailyOutcomesRemaining":494,"nActiveOutcomesRemaining":1}"#).unwrap();
    let limits = DeployerLimits::new(remaining, 99);
    assert_eq!((limits.daily_remaining, limits.active_remaining, limits.active_used), (494, 1, 99));
    assert_eq!(limits.active_cap(), 100);
    let txyz = DeployerLimits { daily_remaining: 500, active_remaining: 27, active_used: 73 };
    let skew = DeployerLimits { daily_remaining: 495, active_remaining: 88, active_used: 12 };
    assert_eq!(txyz.active_cap(), skew.active_cap());
    assert_eq!(serde_json::from_str::<Remaining>("{}").unwrap(), Remaining { daily: 0, active: 0 });
}
