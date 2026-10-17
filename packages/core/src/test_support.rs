//! What the unit tests share: running a future without a runtime, a [`Transport`] that answers
//! from a recording, and an [`Http`] wire that answers from a queue. Nothing leaves the process.

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::{Http, Request, Response, Transport, TransportError};

/// Runs a future to its end on this thread. The fakes here never wait, so each future is ready
/// on its first poll.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// A [`Transport`] that answers every post the same way and keeps each one.
pub(crate) struct Recorded {
    answer: Result<String, TransportError>,
    sent: RefCell<Vec<(String, String)>>,
}

impl Recorded {
    pub(crate) fn answering(text: &str) -> Recorded {
        Recorded { answer: Ok(text.to_owned()), sent: RefCell::default() }
    }

    pub(crate) fn failing(error: TransportError) -> Recorded {
        Recorded { answer: Err(error), sent: RefCell::default() }
    }

    /// Each post: its path and its body.
    pub(crate) fn sent(&self) -> Vec<(String, String)> {
        self.sent.borrow().clone()
    }
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(self.answer.clone())
    }
}

/// An [`Http`] wire that answers from a queue, then with its fallback, and keeps every request.
pub(crate) struct Wire {
    answers: RefCell<VecDeque<Result<Response, TransportError>>>,
    otherwise: Result<Response, TransportError>,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    /// Answers with `answers` in turn, then as unreachable.
    pub(crate) fn answering(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Wire {
        Wire {
            answers: RefCell::new(answers.into_iter().collect()),
            otherwise: Err(TransportError::Unreachable("dry".to_owned())),
            sent: RefCell::default(),
        }
    }

    /// Answers every request `200` with `text`.
    pub(crate) fn always(text: &str) -> Wire {
        Wire::answering([]).then(ok(text))
    }

    /// The same wire, answering `otherwise` once its queue is empty.
    pub(crate) fn then(self, otherwise: Result<Response, TransportError>) -> Wire {
        Wire { otherwise, ..self }
    }

    /// Every request, in order.
    pub(crate) fn sent(&self) -> Vec<Request> {
        self.sent.borrow().clone()
    }

    /// Every request as its URL and its body's text.
    pub(crate) fn bodies(&self) -> Vec<(String, String)> {
        self.sent()
            .into_iter()
            .map(|request| (request.url, String::from_utf8_lossy(&request.body).into_owned()))
            .collect()
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(self.answers.borrow_mut().pop_front().unwrap_or_else(|| self.otherwise.clone()))
    }
}

/// A `200` with `text` as its body.
pub(crate) fn ok(text: &str) -> Result<Response, TransportError> {
    Ok(Response { status: 200, headers: Vec::new(), body: text.as_bytes().to_vec() })
}
