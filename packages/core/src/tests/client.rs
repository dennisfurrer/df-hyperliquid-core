use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use serde::Deserialize;

use super::*;

/// Answers every post with the same text and keeps what was sent. Nothing
/// leaves the process.
struct Recorded {
    answer: Result<String, TransportError>,
    sent: RefCell<Vec<(String, String)>>,
}

impl Recorded {
    fn answering(text: &str) -> Self {
        Recorded { answer: Ok(text.to_owned()), sent: RefCell::new(Vec::new()) }
    }

    fn failing(error: TransportError) -> Self {
        Recorded { answer: Err(error), sent: RefCell::new(Vec::new()) }
    }
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(self.answer.clone())
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn an_account_read_names_its_type_and_user() {
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    assert_eq!(
        user_request("userAbstraction", user),
        r#"{"type":"userAbstraction","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414"}"#
    );
}

#[test]
fn an_info_read_posts_to_info_and_reads_the_answer() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Answer {
        fee: crate::Dec,
    }
    let transport = Recorded::answering(r#"{"fee":"1.0","unknown":true}"#);
    let read: Answer = block_on(info(&transport, r#"{"type":"x"}"#.to_owned())).unwrap();
    assert_eq!(read, Answer { fee: crate::Dec::ONE });
    assert_eq!(transport.sent.borrow().as_slice(), [("/info".to_owned(), r#"{"type":"x"}"#.to_owned())]);

    let garbled = Recorded::answering("<html>busy</html>");
    let error = block_on(info::<Answer>(&garbled, "{}".to_owned())).unwrap_err();
    assert!(matches!(error, ApiError::Unreadable { path: "/info", .. }), "{error}");
}

#[test]
fn a_transfer_accepted_answers_default_with_no_statuses() {
    let transport = Recorded::answering(r#"{"status":"ok","response":{"type":"default"}}"#);
    let accepted = block_on(exchange(&transport, "{}".to_owned())).unwrap();
    assert_eq!(accepted, Accepted { kind: "default".to_owned(), statuses: Vec::new() });
    assert_eq!(accepted.errors().count(), 0);
    assert_eq!(transport.sent.borrow()[0].0, "/exchange");
}

#[test]
fn a_refusal_is_the_venue_s_own_words() {
    let transport = Recorded::answering(r#"{"status":"err","response":"Insufficient balance for withdrawal"}"#);
    let error = block_on(exchange(&transport, "{}".to_owned())).unwrap_err();
    assert_eq!(error, ApiError::Refused("Insufficient balance for withdrawal".to_owned()));
    assert_eq!(error.to_string(), "Insufficient balance for withdrawal");
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

#[test]
fn an_exchange_post_is_made_once_even_when_no_answer_comes() {
    let transport = Recorded::failing(TransportError::Unreachable("timed out".to_owned()));
    let error = block_on(exchange(&transport, r#"{"action":{}}"#.to_owned())).unwrap_err();
    assert_eq!(error, ApiError::Transport(TransportError::Unreachable("timed out".to_owned())));
    assert_eq!(transport.sent.borrow().len(), 1, "never retried");
}
