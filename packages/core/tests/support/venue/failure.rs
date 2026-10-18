//! Failures in place of the venue's answer, for the next request on a path: an HTTP status, a
//! 429 with its wait, or no answer at all. The request is still taken and checked.

use df_hyperliquid_core::client::{Response, TransportError};

use super::{Answer, Venue};

/// An answer in place of the venue's own, for the next request whose URL ends in `path`.
pub(super) struct Failure {
    path: String,
    answer: Answer,
}

impl Venue {
    /// Answers the next request to `path` (`/info`, `/exchange`, or HyperEVM's `/evm`) with HTTP
    /// `status` and `body` in place of the venue's own answer.
    #[must_use]
    pub fn status(self, path: &str, status: u16, body: &str) -> Venue {
        self.failing(path, Ok(Response { status, headers: Vec::new(), body: body.as_bytes().to_vec() }))
    }

    /// Answers the next request to `path` with a 429 whose `Retry-After` asks for `seconds`.
    #[must_use]
    pub fn retry_after(self, path: &str, seconds: u64) -> Venue {
        let headers = vec![("Retry-After".to_owned(), seconds.to_string())];
        self.failing(path, Ok(Response { status: 429, headers, body: b"null".to_vec() }))
    }

    /// The next request to `path` is taken but its answer never comes back: the wire reports it
    /// unreachable. An action is checked and kept, as one that may have landed.
    #[must_use]
    pub fn unreachable(self, path: &str) -> Venue {
        self.failing(path, Err(TransportError::Unreachable("the connection was reset".to_owned())))
    }

    fn failing(self, path: &str, answer: Answer) -> Venue {
        self.failures.borrow_mut().push_back(Failure { path: path.to_owned(), answer });
        self
    }

    /// The failure queued first for this URL, taken.
    pub(super) fn failure(&self, url: &str) -> Option<Answer> {
        let mut failures = self.failures.borrow_mut();
        let at = failures.iter().position(|failure| url.ends_with(&failure.path))?;
        failures.remove(at).map(|failure| failure.answer)
    }
}
