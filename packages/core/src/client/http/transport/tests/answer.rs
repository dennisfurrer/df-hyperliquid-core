//! `/exchange` answers as the venue writes them: accepted, with items refused inside, or
//! unreadable.

use super::*;

#[test]
fn an_answer_s_data_is_kept_whole_beside_its_statuses() {
    let text = r#"{"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":7}}],"oid":7}}}"#;
    let ExchangeResponse::Ok(accepted) = ExchangeResponse::parse(text).unwrap() else { panic!("refused") };
    assert_eq!(accepted.statuses, [Status::Ok(serde_json::json!({"resting": {"oid": 7}}))]);
    let data = accepted.data.unwrap();
    assert_eq!(data["oid"].as_u64(), Some(7), "more than the statuses");
    assert_eq!(data["statuses"][0]["resting"]["oid"].as_u64(), Some(7));
}

#[test]
fn an_item_refused_inside_an_ok_is_surfaced() {
    let answer = r#"{"status":"ok","response":{"type":"order","data":{"statuses":[{"resting":{"oid":77}},{"error":"Order must have minimum value of $10."}]}}}"#;
    let Ok(ExchangeResponse::Ok(accepted)) = ExchangeResponse::parse(answer) else {
        panic!("an ok answer");
    };
    assert_eq!(accepted.kind, "order");
    assert_eq!(accepted.statuses.len(), 2);
    assert_eq!(accepted.errors().collect::<Vec<_>>(), ["Order must have minimum value of $10."]);
    let one = r#"{"status":"ok","response":{"type":"twapOrder","data":{"status":{"error":"too small"}}}}"#;
    let Ok(ExchangeResponse::Ok(single)) = ExchangeResponse::parse(one) else {
        panic!("an ok answer");
    };
    assert_eq!(single.errors().collect::<Vec<_>>(), ["too small"]);
}

#[test]
fn an_answer_without_a_status_is_unreadable_not_a_success() {
    for text in ["", "null", r#"{"response":{"type":"default"}}"#, "Failed to deserialize the JSON body"] {
        assert!(matches!(ExchangeResponse::parse(text), Err(ApiError::Unreadable { path: "/exchange", .. })), "{text}");
    }
}
