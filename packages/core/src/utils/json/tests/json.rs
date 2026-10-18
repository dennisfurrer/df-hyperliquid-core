//! A value's accessors, none of which reads a string as anything else.

use super::*;

#[test]
fn only_true_and_false_are_booleans() {
    let json = Json::parse(r#"{"isSanctioned":false,"merge":true,"n":1,"s":"true","z":null}"#).unwrap();
    assert_eq!(json.get("isSanctioned").and_then(Json::as_bool), Some(false));
    assert_eq!(json.get("merge").and_then(Json::as_bool), Some(true));
    for key in ["n", "s", "z"] {
        assert_eq!(json.get(key).and_then(Json::as_bool), None, "{key}");
    }
}
