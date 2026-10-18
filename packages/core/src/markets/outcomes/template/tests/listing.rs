//! `outcomeTemplates` as the venue answered it: mainnet on 2026-09-28 and testnet on 2026-10-08,
//! trimmed to six and eight templates. Reads through a client are in `tests/mocked/markets`.

use super::*;

const MAINNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));
const TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_testnet.json"));

fn read(text: &str) -> Vec<OutcomeTemplate> {
    serde_json::from_str::<Templates>(text).unwrap().0
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn templates_read_their_roles_and_keywords_as_the_venue_wrote_them() {
    let mainnet = read(MAINNET);
    assert_eq!(mainnet.len(), 6);
    let roles: Vec<(&str, &TemplateRole)> = mainnet.iter().map(|one| (one.id.as_str(), &one.role)).collect();
    let participant = TemplateRole::QuestionOutcome { parent: "sportsTournamentWinner".to_owned() };
    assert_eq!(
        roles,
        [
            ("binaryPrice", &TemplateRole::Standalone { side_names: names(&["Yes", "No"]) }),
            ("priceTouch", &TemplateRole::Standalone { side_names: names(&["Yes", "No"]) }),
            ("sportsContestWinner", &TemplateRole::Standalone { side_names: names(&["{shortNameA}", "{shortNameB}"]) }),
            ("sportsTournamentNoWinner2", &participant),
            ("sportsTournamentParticipant", &participant),
            ("sportsTournamentWinner", &TemplateRole::Question),
        ]
    );
    assert_eq!(mainnet[0].name, "{perp} above {threshold} at {time}?");
    assert_eq!((mainnet[4].parent(), mainnet[5].parent()), (Some("sportsTournamentWinner"), None));
    let testnet = read(TESTNET);
    assert_eq!(testnet.len(), 8);
    let hints: Vec<(&str, Hint)> =
        testnet[0].keywords.iter().map(|keyword| (keyword.name.as_str(), keyword.hint())).collect();
    assert_eq!(
        (testnet[0].id.as_str(), hints),
        (
            "binaryPrice4",
            vec![
                ("perp", Hint::HlPerp),
                ("priceDescription", Hint::String),
                ("seconds", Hint::UInt),
                ("threshold", Hint::UDecimal),
                ("time", Hint::DateTime),
            ]
        )
    );
    let draw = testnet.iter().find(|one| one.id == "sportsContestDraw").unwrap();
    assert_eq!((draw.parent(), draw.keywords.len()), (Some("sportsContestResult"), 0));
}

#[test]
fn a_role_this_build_has_not_seen_is_kept_and_a_template_that_does_not_read_is_skipped() {
    let text = r#"[
        {"id":"somethingNew","role":{"futureRole":{}},"name":"x","description":"","keywords":[]},
        {"id":"oddBody","role":{"standaloneOutcome":7},"name":"x","description":"","keywords":[]},
        {"id":"renamed","role":"answer","name":"x","description":"","keywords":[]},
        {"id":"empty","role":{},"name":"x","description":"","keywords":[]},
        {"id":"bare","name":"x","description":"","keywords":[]},
        {"id":"listed","role":["question"],"name":"x","description":"","keywords":[]},
        {"id":"binaryPrice","role":"question","name":"x","description":"","keywords":[["perp"]]}]"#;
    let roles: Vec<(String, TemplateRole)> = read(text).into_iter().map(|one| (one.id, one.role)).collect();
    assert_eq!(
        roles,
        [
            ("somethingNew".to_owned(), TemplateRole::Other("futureRole".to_owned())),
            ("oddBody".to_owned(), TemplateRole::Standalone { side_names: Vec::new() }),
            ("renamed".to_owned(), TemplateRole::Other("answer".to_owned())),
            ("empty".to_owned(), TemplateRole::Other(String::new())),
            ("bare".to_owned(), TemplateRole::Other(String::new())),
            ("listed".to_owned(), TemplateRole::Other(String::new())),
        ],
        "a keyword with no kind is no template"
    );
    assert!(serde_json::from_str::<Templates>("{}").is_err(), "the answer is a list");
}

/// As outcome 1210 reads on mainnet: the keywords sorted, a standalone outcome's sides under
/// `template:`.
#[test]
fn on_chain_a_row_is_its_template_and_its_keywords_sorted() {
    let templates = read(MAINNET);
    let values = Keywords::from_pairs([
        ("time", "20261001-0000"),
        ("perp", "BTC"),
        ("threshold", "100000"),
        ("seconds", "1"),
        ("priceDescription", "BTC-USDC mark"),
    ])
    .unwrap();
    let chain = templates[0].on_chain(&values);
    assert_eq!(chain.name, "template:binaryPrice");
    assert_eq!(
        chain.description,
        "perp:BTC|priceDescription:BTC-USDC mark|seconds:1|threshold:100000|time:20261001-0000"
    );
    assert_eq!(chain.sides, Some(["template:Yes".to_owned(), "template:No".to_owned()]));
    let participant = templates[4].on_chain(&Keywords::from_pairs([("participant", "Arsenal")]).unwrap());
    assert_eq!(
        (participant.name.as_str(), participant.description.as_str()),
        ("template:sportsTournamentParticipant", "participant:Arsenal")
    );
    assert_eq!(participant.sides, None, "a question's outcome has no sides of its own");
    assert_eq!(
        templates[0].on_chain(&Keywords::new()).description,
        "perp:|priceDescription:|seconds:|threshold:|time:"
    );
}
