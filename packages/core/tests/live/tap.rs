//! A mainnet [`Client`] whose wire keeps every answer, so a test can hold what the venue said
//! against the recording the unit and mocked suites read.

use std::cell::RefCell;
use std::future::Future;
use std::path::PathBuf;

use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::client::{Http, Request, Response, TransportError};
use df_hyperliquid_core::{Client, Network};
use serde_json::Value;

use crate::shape::added;
use crate::support::{entry, fixture};

/// The real wire, keeping each request's body and its answer's text.
pub struct Tap {
    wire: Native,
    kept: RefCell<Vec<(String, String)>>,
}

impl Http for Tap {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        let body = String::from_utf8_lossy(&request.body).into_owned();
        async move {
            let answer = self.wire.send(request).await?;
            self.kept.borrow_mut().push((body, answer.text()));
            Ok(answer)
        }
    }
}

/// A reader on mainnet through a [`Tap`].
pub fn reader() -> Client<Tap> {
    Client::reader(Tap { wire: Native::new(), kept: RefCell::default() }, Network::Mainnet, clock)
}

/// The last answer the client got, as JSON.
pub fn last(client: &Client<Tap>) -> Value {
    let kept = client.http().kept.borrow();
    let (body, answer) = kept.last().expect("nothing was sent");
    serde_json::from_str(answer).unwrap_or_else(|error| panic!("{error}: the answer to {body} is not JSON: {answer}"))
}

/// Holds the last answer against a recording: `fixtures/<path>`, or one entry of it as
/// `<path>#<key>`. Fails naming every key path the venue has added since it was recorded, and
/// keeps the live answer as `target/tmp/live/<path>` to re-record from.
pub fn same_shape(client: &Client<Tap>, recording: &str) {
    let live = last(client);
    let recorded = match recording.split_once('#') {
        Some((path, key)) => entry(path, key),
        None => fixture(recording),
    };
    let recorded: Value = serde_json::from_str(&recorded).unwrap();
    keep(recording, &live);
    let added = added(&recorded, &live);
    assert!(added.is_empty(), "the venue now answers {recording} with fields it did not have: {added:#?}");
}

fn keep(recording: &str, live: &Value) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("live").join(recording.replace('#', "."));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string_pretty(live).unwrap()).unwrap();
}
