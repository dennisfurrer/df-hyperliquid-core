//! `/info` bodies in the order given, and each call made once against a recorded transport.

use serde::Deserialize;

use super::*;
use crate::test_support::{Recorded, block_on};

#[test]
fn an_account_read_names_its_type_and_user() {
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    assert_eq!(
        user_request("userAbstraction", user),
        r#"{"type":"userAbstraction","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414"}"#
    );
}

/// The type first, then each field in the order given, as the docs list
/// them: what `JSON.stringify` writes for the same object.
#[test]
fn an_info_body_keeps_its_fields_in_the_order_given() {
    assert_eq!(info_request("meta", &[]), r#"{"type":"meta"}"#);
    assert_eq!(
        info_request(
            "candleSnapshot",
            &[(
                "req",
                Json::Object(vec![
                    ("coin".to_owned(), Json::String("BTC".to_owned())),
                    ("startTime".to_owned(), Json::Number("1".to_owned())),
                ]),
            )],
        ),
        r#"{"type":"candleSnapshot","req":{"coin":"BTC","startTime":1}}"#
    );
    assert_eq!(
        info_request(
            "userFillsByTime",
            &[
                ("user", Json::String("0x0d1d9635d0640821d15e323ac8adadfa9c111414".to_owned())),
                ("startTime", Json::Number("1790412105001".to_owned())),
                ("aggregateByTime", Json::Bool(true)),
                ("dex", Json::String("x\"y".to_owned())),
            ],
        ),
        concat!(
            r#"{"type":"userFillsByTime","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","#,
            r#""startTime":1790412105001,"aggregateByTime":true,"dex":"x\"y"}"#
        )
    );
}

#[test]
fn an_account_read_is_an_info_body_with_one_user() {
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    assert_eq!(
        user_request("clearinghouseState", user),
        info_request("clearinghouseState", &[("user", Json::String(user.to_hex()))])
    );
}

#[test]
fn an_info_read_posts_to_info_and_reads_the_answer() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Answer {
        fee: crate::utils::Dec,
    }
    let transport = Recorded::answering(r#"{"fee":"1.0","unknown":true}"#);
    let read: Answer = block_on(info(&transport, r#"{"type":"x"}"#.to_owned())).unwrap();
    assert_eq!(read, Answer { fee: crate::utils::Dec::ONE });
    assert_eq!(transport.sent(), [("/info".to_owned(), r#"{"type":"x"}"#.to_owned())]);

    let garbled = Recorded::answering("<html>busy</html>");
    let error = block_on(info::<Answer>(&garbled, "{}".to_owned())).unwrap_err();
    assert!(matches!(error, ApiError::Unreadable { path: "/info", .. }), "{error}");
}

#[test]
fn a_transfer_accepted_answers_default_with_no_statuses() {
    let transport = Recorded::answering(r#"{"status":"ok","response":{"type":"default"}}"#);
    let accepted = block_on(exchange(&transport, "{}".to_owned())).unwrap();
    assert_eq!(accepted, Accepted { kind: "default".to_owned(), statuses: Vec::new(), data: None });
    assert_eq!(accepted.errors().count(), 0);
    assert_eq!(transport.sent()[0].0, "/exchange");
}

#[test]
fn a_refusal_is_the_venue_s_own_words() {
    let transport = Recorded::answering(r#"{"status":"err","response":"Insufficient balance for withdrawal"}"#);
    let error = block_on(exchange(&transport, "{}".to_owned())).unwrap_err();
    assert_eq!(error, ApiError::Refused("Insufficient balance for withdrawal".to_owned()));
    assert_eq!(error.to_string(), "Insufficient balance for withdrawal");
}

#[test]
fn an_exchange_post_is_made_once_even_when_no_answer_comes() {
    let transport = Recorded::failing(TransportError::Unreachable("timed out".to_owned()));
    let error = block_on(exchange(&transport, r#"{"action":{}}"#.to_owned())).unwrap_err();
    assert_eq!(error, ApiError::Transport(TransportError::Unreachable("timed out".to_owned())));
    assert_eq!(transport.sent().len(), 1, "never retried");
}
