//! Msgpack as the venue hashes an L1 action: map keys in insertion order,
//! never sorted, and a non-negative integer always unsigned.

use std::borrow::Cow;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

use crate::utils::json::Json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Nil,
    Bool(bool),
    Uint(u64),
    /// A negative integer. Non-negative values still encode as unsigned.
    Int(i64),
    Str(Cow<'static, str>),
    Array(Vec<Value>),
    Map(Map),
}

impl Value {
    #[must_use]
    pub fn array<V: Into<Value>>(items: impl IntoIterator<Item = V>) -> Value {
        Value::Array(items.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub fn to_msgpack(&self) -> Vec<u8> {
        encode(self)
    }

    /// The same tree as JSON, keys in the same order: what is posted is what
    /// was hashed.
    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Value::Nil => Json::Null,
            Value::Bool(value) => Json::Bool(*value),
            Value::Uint(number) => Json::Number(number.to_string()),
            Value::Int(number) => Json::Number(number.to_string()),
            Value::Str(text) => Json::String(text.to_string()),
            Value::Array(items) => Json::Array(items.iter().map(Value::to_json).collect()),
            Value::Map(map) => map.to_json(),
        }
    }
}

/// An ordered msgpack map: a list of key/value pairs in wire order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Map(Vec<(Cow<'static, str>, Value)>);

impl Map {
    #[must_use]
    pub const fn new() -> Self {
        Map(Vec::new())
    }

    #[must_use]
    pub fn put(mut self, key: impl Into<Cow<'static, str>>, value: impl Into<Value>) -> Self {
        self.0.push((key.into(), value.into()));
        self
    }

    #[must_use]
    pub fn put_some(self, key: impl Into<Cow<'static, str>>, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(value) => self.put(key, value),
            None => self,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The pairs themselves, in wire order, for splicing one map into another.
    pub fn into_pairs(self) -> impl Iterator<Item = (Cow<'static, str>, Value)> {
        self.0.into_iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(key, _)| key.as_ref())
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(stored, _)| stored == key).map(|(_, value)| value)
    }

    #[must_use]
    pub fn to_msgpack(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_map(self, &mut out);
        out
    }

    #[must_use]
    pub fn to_json(&self) -> Json {
        Json::Object(self.0.iter().map(|(key, value)| (key.to_string(), value.to_json())).collect())
    }
}

impl From<Map> for Value {
    fn from(map: Map) -> Value {
        Value::Map(map)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Value {
        Value::Bool(value)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Value {
        Value::Uint(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Value {
        Value::Uint(u64::from(value))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Value {
        Value::Int(value)
    }
}

impl From<&'static str> for Value {
    fn from(value: &'static str) -> Value {
        Value::Str(Cow::Borrowed(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Value {
        Value::Str(Cow::Owned(value))
    }
}

impl From<Cow<'static, str>> for Value {
    fn from(value: Cow<'static, str>) -> Value {
        Value::Str(value)
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Value {
        Value::Array(value)
    }
}

#[must_use]
pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

pub fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Nil => out.push(0xc0),
        Value::Bool(false) => out.push(0xc2),
        Value::Bool(true) => out.push(0xc3),
        Value::Uint(number) => write_uint(*number, out),
        Value::Int(number) => write_int(*number, out),
        Value::Str(text) => write_str(text, out),
        Value::Array(items) => {
            write_len(items.len(), 0x90, 0xdc, 0xdd, out);
            for item in items {
                write(item, out);
            }
        }
        Value::Map(map) => write_map(map, out),
    }
}

fn write_map(map: &Map, out: &mut Vec<u8>) {
    write_len(map.0.len(), 0x80, 0xde, 0xdf, out);
    for (key, value) in &map.0 {
        write_str(key, out);
        write(value, out);
    }
}

fn write_len(len: usize, fixed: u8, wide16: u8, wide32: u8, out: &mut Vec<u8>) {
    if len <= 15 {
        out.push(fixed | len as u8);
    } else if let Ok(len) = u16::try_from(len) {
        out.push(wide16);
        out.extend_from_slice(&len.to_be_bytes());
    } else {
        out.push(wide32);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
}

fn write_str(text: &str, out: &mut Vec<u8>) {
    let bytes = text.as_bytes();
    if bytes.len() <= 31 {
        out.push(0xa0 | bytes.len() as u8);
    } else if let Ok(len) = u8::try_from(bytes.len()) {
        out.push(0xd9);
        out.push(len);
    } else if let Ok(len) = u16::try_from(bytes.len()) {
        out.push(0xda);
        out.extend_from_slice(&len.to_be_bytes());
    } else {
        out.push(0xdb);
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    }
    out.extend_from_slice(bytes);
}

fn write_uint(number: u64, out: &mut Vec<u8>) {
    if number <= 0x7f {
        out.push(number as u8);
    } else if let Ok(byte) = u8::try_from(number) {
        out.push(0xcc);
        out.push(byte);
    } else if let Ok(short) = u16::try_from(number) {
        out.push(0xcd);
        out.extend_from_slice(&short.to_be_bytes());
    } else if let Ok(word) = u32::try_from(number) {
        out.push(0xce);
        out.extend_from_slice(&word.to_be_bytes());
    } else {
        out.push(0xcf);
        out.extend_from_slice(&number.to_be_bytes());
    }
}

fn write_int(number: i64, out: &mut Vec<u8>) {
    if let Ok(unsigned) = u64::try_from(number) {
        write_uint(unsigned, out);
    } else if number >= -32 {
        out.push(number as u8);
    } else if let Ok(byte) = i8::try_from(number) {
        out.push(0xd0);
        out.extend_from_slice(&byte.to_be_bytes());
    } else if let Ok(short) = i16::try_from(number) {
        out.push(0xd1);
        out.extend_from_slice(&short.to_be_bytes());
    } else if let Ok(word) = i32::try_from(number) {
        out.push(0xd2);
        out.extend_from_slice(&word.to_be_bytes());
    } else {
        out.push(0xd3);
        out.extend_from_slice(&number.to_be_bytes());
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Nil => serializer.serialize_unit(),
            Value::Bool(value) => serializer.serialize_bool(*value),
            Value::Uint(number) => serializer.serialize_u64(*number),
            Value::Int(number) => serializer.serialize_i64(*number),
            Value::Str(text) => serializer.serialize_str(text),
            Value::Array(items) => {
                let mut sequence = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    sequence.serialize_element(item)?;
                }
                sequence.end()
            }
            Value::Map(map) => map.serialize(serializer),
        }
    }
}

impl Serialize for Map {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut entries = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            entries.serialize_entry(key.as_ref(), value)?;
        }
        entries.end()
    }
}

#[cfg(test)]
#[path = "tests/msgpack.rs"]
mod tests;
