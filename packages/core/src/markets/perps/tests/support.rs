//! Test helpers: a transport that answers from recordings by the request's `type`, and a way to
//! run a future to its end.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::{Transport, TransportError};
use crate::utils::{Dec, Json};

/// Answers each `/info` request with the first recording left for its `type`, `null` once none
/// is, and keeps every body it was sent.
pub struct Recorded {
    answers: RefCell<Vec<(String, String)>>,
    pub sent: RefCell<Vec<String>>,
}

impl Recorded {
    pub fn new(answers: &[(&str, &str)]) -> Recorded {
        Recorded {
            answers: RefCell::new(
                answers.iter().map(|(kind, text)| ((*kind).to_owned(), (*text).to_owned())).collect(),
            ),
            sent: RefCell::new(Vec::new()),
        }
    }

    pub fn sent(&self) -> Vec<String> {
        self.sent.borrow().clone()
    }
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        assert_eq!(path, "/info", "a read never goes to /exchange");
        let kind = Json::parse(&body).unwrap().get("type").and_then(Json::as_str).unwrap().to_owned();
        let mut answers = self.answers.borrow_mut();
        let answer = match answers.iter().position(|(recorded, _)| *recorded == kind) {
            Some(at) => answers.remove(at).1,
            None => "null".to_owned(),
        };
        self.sent.borrow_mut().push(body);
        ready(Ok(answer))
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

pub fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}
