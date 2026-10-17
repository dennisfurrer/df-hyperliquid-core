//! What the read tests share: a transport that answers from a recording and
//! keeps what was posted, and a way to run a future without a runtime.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::{Transport, TransportError};

/// Answers every post with one recorded answer. Nothing leaves the process.
pub struct Recorded {
    answer: String,
    sent: RefCell<Vec<(String, String)>>,
}

impl Recorded {
    pub fn answering(answer: &str) -> Recorded {
        Recorded { answer: answer.to_owned(), sent: RefCell::new(Vec::new()) }
    }

    /// Each post: its path and its body.
    pub fn sent(&self) -> Vec<(String, String)> {
        self.sent.borrow().clone()
    }
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(Ok(self.answer.clone()))
    }
}

/// The futures here never wait: each resolves on its first poll.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}
