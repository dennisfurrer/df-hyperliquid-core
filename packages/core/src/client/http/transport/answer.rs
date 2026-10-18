//! What `/exchange` answers: an accepted action with its per-item statuses, or
//! the venue's refusal in its own words.

use serde_json::Value;

use crate::client::http::transport::calls::unreadable;
use crate::client::http::transport::{ApiError, EXCHANGE};

/// What `/exchange` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeResponse {
    /// `{"status":"ok","response":{...}}`.
    Ok(Accepted),
    /// `{"status":"err","response":"message"}`.
    Err(String),
}

/// An accepted action. `kind` is `response.type`: `"default"` for a transfer,
/// `"order"` and the like for actions that answer per item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub kind: String,
    /// `response.data.statuses`, or `response.data.status` alone, when present.
    pub statuses: Vec<Status>,
    /// `response.data` whole, as the venue wrote it; `None` when there is none
    /// (`"default"`). Some answers carry more than their statuses.
    pub data: Option<Value>,
}

/// One item of an accepted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok(Value),
    /// `{"error": "..."}`: that item was refused though the action was taken.
    Error(String),
}

impl Accepted {
    /// The items the venue refused inside an `ok`.
    pub fn errors(&self) -> impl Iterator<Item = &str> {
        self.statuses.iter().filter_map(|status| match status {
            Status::Error(reason) => Some(reason.as_str()),
            Status::Ok(_) => None,
        })
    }
}

impl ExchangeResponse {
    pub fn parse(text: &str) -> Result<ExchangeResponse, ApiError> {
        let bad = |detail: String| ApiError::Unreadable { path: EXCHANGE, detail };
        let answer: Value = serde_json::from_str(text).map_err(|error| bad(unreadable(&error, text)))?;
        let response = answer.get("response").cloned().unwrap_or(Value::Null);
        match answer.get("status").and_then(Value::as_str) {
            Some("ok") => {
                let kind = response.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
                let data = response.get("data");
                let statuses = match (
                    data.and_then(|data| data.get("statuses")).and_then(Value::as_array),
                    data.and_then(|data| data.get("status")),
                ) {
                    (Some(many), _) => many.iter().map(status_of).collect(),
                    (None, Some(one)) => vec![status_of(one)],
                    (None, None) => Vec::new(),
                };
                Ok(ExchangeResponse::Ok(Accepted { kind, statuses, data: data.cloned() }))
            }
            Some(_) => Ok(ExchangeResponse::Err(text_of(&response))),
            None => Err(bad(format!("no status in {}", text.chars().take(300).collect::<String>()))),
        }
    }
}

fn status_of(status: &Value) -> Status {
    match status.get("error") {
        Some(error) => Status::Error(text_of(error)),
        None => Status::Ok(status.clone()),
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().map_or_else(|| value.to_string(), ToOwned::to_owned)
}

#[cfg(test)]
#[path = "tests/answer.rs"]
mod tests;
