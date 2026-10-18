//! JSON as the msgpack an L1 signature is over: keys in the order written, integers as the venue's
//! own clients pack them, and never a float.

use crate::utils::json::Json;
use crate::utils::sign::msgpack::{Map, Value};

/// Why a JSON value has no msgpack form the venue would hash the same way.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unpackable {
    #[error("{0} is not a whole number; the venue hashes decimals as strings, so write it in quotes")]
    Fraction(String),
    #[error("{0} does not fit in 64 bits")]
    TooWide(String),
    #[error("an action is an object, and this is {0}")]
    NotAnObject(&'static str),
}

impl Json {
    /// The msgpack value this hashes as, keys in the order they were written.
    pub fn to_msgpack(&self) -> Result<Value, Unpackable> {
        Ok(match self {
            Json::Null => Value::Nil,
            Json::Bool(value) => Value::Bool(*value),
            Json::Number(digits) => whole(digits)?,
            Json::String(text) => Value::from(text.clone()),
            Json::Array(items) => Value::Array(items.iter().map(Json::to_msgpack).collect::<Result<_, _>>()?),
            Json::Object(entries) => Value::Map(map_of(entries)?),
        })
    }

    /// The action map an L1 signature is over. Only an object is one.
    pub fn to_action(&self) -> Result<Map, Unpackable> {
        match self {
            Json::Object(entries) => map_of(entries),
            other => Err(Unpackable::NotAnObject(other.kind())),
        }
    }
}

fn map_of(entries: &[(String, Json)]) -> Result<Map, Unpackable> {
    let mut map = Map::new();
    for (key, value) in entries {
        map = map.put(key.clone(), value.to_msgpack()?);
    }
    Ok(map)
}

/// An integer as msgpack encodes it: unsigned when it can be, as the venue's
/// own clients do. A fraction or an exponent would be a float, and a float is
/// never what an action means.
fn whole(digits: &str) -> Result<Value, Unpackable> {
    if digits.contains(['.', 'e', 'E']) {
        return Err(Unpackable::Fraction(digits.to_owned()));
    }
    match digits.strip_prefix('-') {
        Some(_) => digits.parse::<i64>().map(Value::Int).map_err(|_| Unpackable::TooWide(digits.to_owned())),
        None => digits.parse::<u64>().map(Value::Uint).map_err(|_| Unpackable::TooWide(digits.to_owned())),
    }
}

#[cfg(test)]
#[path = "tests/msgpack.rs"]
mod tests;
