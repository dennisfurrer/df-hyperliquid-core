//! `/info` bodies in the order given, and a read or an action posted once to its path.

use super::*;
use crate::test_support::{Recorded, block_on};

/// The type first, then each field in the order given, as the docs list them: what
/// `JSON.stringify` writes for the same object.
#[test]
fn an_info_body_keeps_its_fields_in_the_order_given() {
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    let candle = Json::Object(vec![
        ("coin".to_owned(), Json::String("BTC".to_owned())),
        ("startTime".to_owned(), Json::Number("1".to_owned())),
    ]);
    let fills = [
        ("user", Json::String(user.to_hex())),
        ("startTime", Json::Number("1790412105001".to_owned())),
        ("aggregateByTime", Json::Bool(true)),
        ("dex", Json::String("x\"y".to_owned())),
    ];
    for (body, expected) in [
        (info_request("meta", &[]), r#"{"type":"meta"}"#),
        (
            info_request("candleSnapshot", &[("req", candle)]),
            r#"{"type":"candleSnapshot","req":{"coin":"BTC","startTime":1}}"#,
        ),
        (
            info_request("userFillsByTime", &fills),
            r#"{"type":"userFillsByTime","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","startTime":1790412105001,"aggregateByTime":true,"dex":"x\"y"}"#,
        ),
        (
            user_request("userAbstraction", user),
            r#"{"type":"userAbstraction","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414"}"#,
        ),
    ] {
        assert_eq!(body, expected);
    }
}

#[test]
fn a_read_and_an_action_are_each_posted_once_to_their_path() {
    let read = Recorded::answering("<html>busy</html>");
    let error = block_on(info::<serde_json::Value>(&read, r#"{"type":"meta"}"#.to_owned())).unwrap_err();
    assert!(matches!(error, ApiError::Unreadable { path: "/info", .. }), "{error}");
    assert_eq!(read.sent(), [("/info".to_owned(), r#"{"type":"meta"}"#.to_owned())]);
    let lost = Recorded::failing(TransportError::Unreachable("timed out".to_owned()));
    let error = block_on(exchange(&lost, r#"{"action":{}}"#.to_owned())).unwrap_err();
    assert_eq!(error, ApiError::Transport(TransportError::Unreachable("timed out".to_owned())));
    assert_eq!(lost.sent(), [("/exchange".to_owned(), r#"{"action":{}}"#.to_owned())], "never again");
}
