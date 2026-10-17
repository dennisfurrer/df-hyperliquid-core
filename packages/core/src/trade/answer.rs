//! What `/exchange` answers to a trading action, read per item. A count of statuses that does not
//! match what was sent is unreadable, never a partial success, except an error decided by the
//! payload alone, which comes back once and stands for every item.

use crate::client::http::transport::EXCHANGE;
use crate::client::{Accepted, ApiError, ExchangeResponse, Status, Transport, exchange};
use crate::trade::orders::Cloid;
use crate::utils::Dec;
use serde::Deserialize;
use serde_json::Value;

/// What the venue said of one order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderAnswer {
    /// On the book.
    Resting { oid: u64, cloid: Option<Cloid> },
    /// Filled `total_sz` at `avg_px` on average: all of it, or as much as
    /// crossed for an IOC or a market order.
    Filled { oid: u64, total_sz: Dec, avg_px: Dec, cloid: Option<Cloid> },
    /// A take profit or stop loss, placed and waiting for its trigger.
    WaitingForTrigger,
    /// A take profit or stop loss grouped with an entry that has not filled.
    WaitingForFill,
    /// Refused, in the venue's words.
    Refused(String),
    /// A status this build does not know, kept as written. Never read as placed.
    Unknown(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rested {
    oid: u64,
    #[serde(default)]
    cloid: Option<Cloid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filled {
    oid: u64,
    total_sz: Dec,
    avg_px: Dec,
    #[serde(default)]
    cloid: Option<Cloid>,
}

impl OrderAnswer {
    /// One order's status. An object this build knows but cannot read (a
    /// resting order with no `oid`) is an error, not a guess.
    pub fn read(status: &Status) -> Result<OrderAnswer, String> {
        let value = match status {
            Status::Error(reason) => return Ok(OrderAnswer::Refused(reason.clone())),
            Status::Ok(value) => value,
        };
        let shape = |error: serde_json::Error| format!("{error} in {value}");
        match value {
            Value::String(word) => Ok(match word.as_str() {
                "waitingForTrigger" => OrderAnswer::WaitingForTrigger,
                "waitingForFill" => OrderAnswer::WaitingForFill,
                other => OrderAnswer::Unknown(other.to_owned()),
            }),
            Value::Object(entry) => {
                if let Some(resting) = entry.get("resting") {
                    let Rested { oid, cloid } = Rested::deserialize(resting).map_err(shape)?;
                    return Ok(OrderAnswer::Resting { oid, cloid });
                }
                if let Some(filled) = entry.get("filled") {
                    let Filled { oid, total_sz, avg_px, cloid } = Filled::deserialize(filled).map_err(shape)?;
                    return Ok(OrderAnswer::Filled { oid, total_sz, avg_px, cloid });
                }
                Ok(OrderAnswer::Unknown(value.to_string()))
            }
            other => Err(format!("a status that is neither a word nor an object: {other}")),
        }
    }

    /// The venue's id for the order, once it has one.
    #[must_use]
    pub const fn oid(&self) -> Option<u64> {
        match self {
            OrderAnswer::Resting { oid, .. } | OrderAnswer::Filled { oid, .. } => Some(*oid),
            _ => None,
        }
    }

    /// Taken by the venue: resting, filled, or waiting.
    #[must_use]
    pub const fn is_placed(&self) -> bool {
        matches!(
            self,
            OrderAnswer::Resting { .. }
                | OrderAnswer::Filled { .. }
                | OrderAnswer::WaitingForTrigger
                | OrderAnswer::WaitingForFill
        )
    }

    #[must_use]
    pub fn refusal(&self) -> Option<&str> {
        match self {
            OrderAnswer::Refused(reason) => Some(reason),
            _ => None,
        }
    }
}

/// What the venue said of one cancel, or of a TWAP's cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelAnswer {
    Cancelled,
    /// In the venue's words; any word but `success` is a refusal.
    Refused(String),
}

impl CancelAnswer {
    #[must_use]
    pub fn read(status: &Status) -> CancelAnswer {
        match status {
            Status::Error(reason) => CancelAnswer::Refused(reason.clone()),
            Status::Ok(Value::String(word)) if word == "success" => CancelAnswer::Cancelled,
            Status::Ok(Value::String(word)) => CancelAnswer::Refused(word.clone()),
            Status::Ok(other) => CancelAnswer::Refused(other.to_string()),
        }
    }
}

/// What the venue said of a TWAP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TwapAnswer {
    /// Running under this id, which `twapCancel` names.
    Running {
        twap_id: u64,
    },
    Refused(String),
}

fn unreadable(detail: String) -> ApiError {
    ApiError::Unreadable { path: EXCHANGE, detail }
}

/// One answer per item sent, the lone error of a whole batch repeated.
fn per_item<T>(
    accepted: &Accepted,
    sent: usize,
    read: impl Fn(&Status) -> Result<T, String>,
) -> Result<Vec<T>, ApiError> {
    let statuses = accepted.statuses.as_slice();
    if let [lone @ Status::Error(_)] = statuses
        && sent > 1
    {
        return (0..sent).map(|_| read(lone).map_err(unreadable)).collect();
    }
    if statuses.len() != sent {
        return Err(unreadable(format!("{sent} were sent and {} statuses came back", statuses.len())));
    }
    statuses.iter().map(|status| read(status).map_err(unreadable)).collect()
}

/// The one status a TWAP or its cancel answers.
fn lone(accepted: &Accepted) -> Result<&Status, ApiError> {
    match accepted.statuses.as_slice() {
        [status] => Ok(status),
        many => Err(unreadable(format!("one status was expected and {} came back", many.len()))),
    }
}

/// An `order` or `batchModify` answer, `sent` orders in it.
pub fn orders(accepted: &Accepted, sent: usize) -> Result<Vec<OrderAnswer>, ApiError> {
    per_item(accepted, sent, OrderAnswer::read)
}

/// A `cancel` or `cancelByCloid` answer, `sent` cancels in it.
pub fn cancels(accepted: &Accepted, sent: usize) -> Result<Vec<CancelAnswer>, ApiError> {
    per_item(accepted, sent, |status| Ok(CancelAnswer::read(status)))
}

/// A `twapOrder` answer.
pub fn twap(accepted: &Accepted) -> Result<TwapAnswer, ApiError> {
    match lone(accepted)? {
        Status::Error(reason) => Ok(TwapAnswer::Refused(reason.clone())),
        Status::Ok(value) => value
            .get("running")
            .and_then(|running| running.get("twapId"))
            .and_then(Value::as_u64)
            .map(|twap_id| TwapAnswer::Running { twap_id })
            .ok_or_else(|| unreadable(format!("a TWAP status this build cannot read: {value}"))),
    }
}

/// A `twapCancel` answer.
pub fn twap_cancel(accepted: &Accepted) -> Result<CancelAnswer, ApiError> {
    lone(accepted).map(CancelAnswer::read)
}

/// An answer that says only that it was taken: a single `modify`, leverage,
/// margin, `scheduleCancel`, `noop`. A status that is an error refuses it.
pub fn taken(accepted: &Accepted) -> Result<(), ApiError> {
    match accepted.errors().next() {
        Some(reason) => Err(ApiError::Refused(reason.to_owned())),
        None => Ok(()),
    }
}

/// The answer's text, read: `ok`, or the venue's refusal of the whole action.
pub fn read(text: &str) -> Result<Accepted, ApiError> {
    match ExchangeResponse::parse(text)? {
        ExchangeResponse::Ok(accepted) => Ok(accepted),
        ExchangeResponse::Err(reason) => Err(ApiError::Refused(reason)),
    }
}

/// Posts a signed `order` or `batchModify` body once and reads its answer.
pub async fn send_orders(transport: &impl Transport, body: String, sent: usize) -> Result<Vec<OrderAnswer>, ApiError> {
    orders(&exchange(transport, body).await?, sent)
}

/// Posts a signed `cancel` or `cancelByCloid` body once and reads its answer.
pub async fn send_cancels(
    transport: &impl Transport,
    body: String,
    sent: usize,
) -> Result<Vec<CancelAnswer>, ApiError> {
    cancels(&exchange(transport, body).await?, sent)
}

/// Posts a signed `twapOrder` body once and reads its answer.
pub async fn send_twap(transport: &impl Transport, body: String) -> Result<TwapAnswer, ApiError> {
    twap(&exchange(transport, body).await?)
}

/// Posts a signed `twapCancel` body once and reads its answer.
pub async fn send_twap_cancel(transport: &impl Transport, body: String) -> Result<CancelAnswer, ApiError> {
    twap_cancel(&exchange(transport, body).await?)
}

/// Posts any other signed trading body once: taken, or refused.
pub async fn send_taken(transport: &impl Transport, body: String) -> Result<(), ApiError> {
    taken(&exchange(transport, body).await?)
}

#[cfg(test)]
#[path = "tests/answer.rs"]
mod tests;
