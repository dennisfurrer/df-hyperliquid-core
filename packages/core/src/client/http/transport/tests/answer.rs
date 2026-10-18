//! `/exchange` answers as the venue writes them: accepted with its items, some refused inside,
//! the venue's refusal in its own words, or unreadable.

use super::*;

#[test]
fn an_answer_reads_its_kind_its_items_and_its_data_whole() {
    let one = r#"{"status":"ok","response":{"type":"twapOrder","data":{"status":{"error":"too small"}}}}"#;
    for (text, kind, statuses, errors) in [
        (r#"{"status":"ok","response":{"type":"default"}}"#, "default", 0, vec![]),
        (
            r#"{"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":77}},{"error":"Order must have minimum value of $10."}]}}}"#,
            "order",
            2,
            vec!["Order must have minimum value of $10."],
        ),
        (one, "twapOrder", 1, vec!["too small"]),
    ] {
        let Ok(ExchangeResponse::Ok(accepted)) = ExchangeResponse::parse(text) else { panic!("{text}") };
        assert_eq!((accepted.kind.as_str(), accepted.statuses.len()), (kind, statuses), "{text}");
        assert_eq!(accepted.errors().collect::<Vec<_>>(), errors, "{text}");
    }
    let text = r#"{"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":7}}],"oid":7}}}"#;
    let Ok(ExchangeResponse::Ok(accepted)) = ExchangeResponse::parse(text) else { panic!("{text}") };
    assert_eq!(accepted.statuses, [Status::Ok(serde_json::json!({"resting": {"oid": 7}}))]);
    assert_eq!(accepted.data.unwrap()["oid"].as_u64(), Some(7), "more than the statuses");
}

#[test]
fn a_refusal_is_the_venue_s_words_and_an_answer_without_a_status_is_unreadable() {
    let refused = r#"{"status":"err","response":"Insufficient balance for withdrawal"}"#;
    assert_eq!(
        ExchangeResponse::parse(refused),
        Ok(ExchangeResponse::Err("Insufficient balance for withdrawal".to_owned()))
    );
    let odd = r#"{"status":"err","response":{"code":7}}"#;
    assert_eq!(ExchangeResponse::parse(odd), Ok(ExchangeResponse::Err(r#"{"code":7}"#.to_owned())), "as written");
    for text in ["", "null", r#"{"response":{"type":"default"}}"#, "Failed to deserialize the JSON body"] {
        assert!(matches!(ExchangeResponse::parse(text), Err(ApiError::Unreadable { path: "/exchange", .. })), "{text}");
    }
}
