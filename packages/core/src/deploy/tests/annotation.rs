//! An annotation as typed: the lengths the docs give, counted in characters, and the keys it adds.

use super::*;

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn an_annotation_keeps_to_the_lengths_the_docs_give_in_characters() {
    let full = Annotation::new("a".repeat(15), "d".repeat(400), Some("n".repeat(9)), words(&["k123456789", "x"]));
    assert!(full.is_ok());
    assert!(Annotation::new("\u{e9}".repeat(15), "", None, vec![]).is_ok(), "characters, not bytes");
    assert_eq!(
        Annotation::new("a".repeat(16), "", None, vec![]),
        Err(BadDeploy::TooLong { what: "a category", most: 15, value: "a".repeat(16) })
    );
    assert!(matches!(
        Annotation::new("", "d".repeat(401), None, vec![]),
        Err(BadDeploy::TooLong { what: "a description", most: 400, .. })
    ));
    assert!(matches!(
        Annotation::new("", "", Some("n".repeat(10)), vec![]),
        Err(BadDeploy::TooLong { what: "a display name", most: 9, .. })
    ));
    assert!(matches!(
        Annotation::new("", "", None, words(&["k1234567890"])),
        Err(BadDeploy::TooLong { what: "a keyword", most: 10, .. })
    ));
    assert_eq!(Annotation::new("", "", None, words(&["a", "b", "c"])), Err(BadDeploy::Keywords(3)));
}

#[test]
fn an_annotation_follows_the_keys_before_it_and_sends_no_display_name_as_null() {
    let annotation = Annotation::new("ai", "Tokens for AI.", None, words(&["ai", "agents"])).unwrap();
    assert_eq!(
        annotation.wire_after(Map::new().put("coin", "xyz:AI")).to_json().compact(),
        r#"{"coin":"xyz:AI","category":"ai","description":"Tokens for AI.","displayName":null,"keywords":["ai","agents"]}"#
    );
}
