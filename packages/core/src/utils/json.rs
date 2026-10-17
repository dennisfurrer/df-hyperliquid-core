//! JSON that keeps what a signature depends on: keys in the order they were
//! written and numbers as the digits they were written with. An L1 action is
//! hashed as msgpack, which keeps key order, where `serde_json` sorts keys.

use std::fmt::Write as _;

use crate::utils::sign::msgpack::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number exactly as written: no float ever holds it.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Keys in the order they were written.
    Object(Vec<(String, Json)>),
}

/// Where the text stopped being JSON, counted from one as an editor counts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}, column {column}: {message}")]
pub struct BadJson {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

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
    pub fn parse(text: &str) -> Result<Json, BadJson> {
        let mut reader = Reader { text: text.as_bytes(), at: 0, source: text };
        reader.space();
        let value = reader.value(0)?;
        reader.space();
        if reader.at < reader.text.len() {
            return Err(reader.fail("more after the value ends"));
        }
        Ok(value)
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    /// `true` or `false`; `None` for anything else, a `"true"` string too.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// A whole non-negative number, as the venue writes counts and ids.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Number(digits) => digits.parse().ok(),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Number(_) => "a number",
            Json::String(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }

    /// One line, no spaces: what `JSON.stringify` writes, byte for byte.
    #[must_use]
    pub fn compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None, 0);
        out
    }

    /// Two spaces a level, for showing a body to a person.
    #[must_use]
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(2), 0);
        out
    }

    fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        let open = |out: &mut String, depth: usize| {
            if let Some(step) = indent {
                out.push('\n');
                out.push_str(&" ".repeat(step * depth));
            }
        };
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Number(digits) => out.push_str(digits),
            Json::String(text) => quote(text, out),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Array(items) => {
                out.push('[');
                for (at, item) in items.iter().enumerate() {
                    if at > 0 {
                        out.push(',');
                    }
                    open(out, depth + 1);
                    item.write(out, indent, depth + 1);
                }
                open(out, depth);
                out.push(']');
            }
            Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Object(entries) => {
                out.push('{');
                for (at, (key, value)) in entries.iter().enumerate() {
                    if at > 0 {
                        out.push(',');
                    }
                    open(out, depth + 1);
                    quote(key, out);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                    value.write(out, indent, depth + 1);
                }
                open(out, depth);
                out.push('}');
            }
        }
    }

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

fn quote(text: &str, out: &mut String) {
    out.push('"');
    for letter in text.chars() {
        match letter {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            control if u32::from(control) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(control));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Nesting deeper than this is refused rather than followed off the stack.
const DEEPEST: usize = 128;

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
    source: &'a str,
}

impl Reader<'_> {
    fn fail(&self, message: &str) -> BadJson {
        let before = self.source.get(..self.at).unwrap_or(self.source);
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |tail| tail.chars().count()) + 1;
        BadJson { line, column, message: message.to_owned() }
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }

    fn expect(&mut self, word: &str) -> Result<(), BadJson> {
        match self.text[self.at..].starts_with(word.as_bytes()) {
            true => {
                self.at += word.len();
                Ok(())
            }
            false => Err(self.fail(&format!("expected {word}"))),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, BadJson> {
        if depth > DEEPEST {
            return Err(self.fail("nested too deep"));
        }
        match self.peek() {
            None => Err(self.fail("the text ends where a value should be")),
            Some(b'n') => self.expect("null").map(|()| Json::Null),
            Some(b't') => self.expect("true").map(|()| Json::Bool(true)),
            Some(b'f') => self.expect("false").map(|()| Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.fail("not the start of a value")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, BadJson> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.space();
            items.push(self.value(depth + 1)?);
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.fail("expected , or ] in an array")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, BadJson> {
        self.at += 1;
        let mut entries: Vec<(String, Json)> = Vec::new();
        self.space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(self.fail("expected a key in quotes"));
            }
            let key = self.string()?;
            if entries.iter().any(|(seen, _)| *seen == key) {
                return Err(self.fail(&format!("the key {key:?} is written twice")));
            }
            self.space();
            if self.peek() != Some(b':') {
                return Err(self.fail("expected : after a key"));
            }
            self.at += 1;
            self.space();
            let value = self.value(depth + 1)?;
            entries.push((key, value));
            self.space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(entries));
                }
                _ => return Err(self.fail("expected , or } in an object")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, BadJson> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        let digits = |reader: &mut Self| {
            let from = reader.at;
            while matches!(reader.peek(), Some(b'0'..=b'9')) {
                reader.at += 1;
            }
            reader.at - from
        };
        let whole = self.at;
        if digits(self) == 0 {
            return Err(self.fail("a number needs digits"));
        }
        if self.text[whole] == b'0' && self.at - whole > 1 {
            return Err(self.fail("a number does not start with 0"));
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if digits(self) == 0 {
                return Err(self.fail("digits must follow the point"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if digits(self) == 0 {
                return Err(self.fail("digits must follow the exponent"));
            }
        }
        Ok(Json::Number(self.source[start..self.at].to_owned()))
    }

    fn string(&mut self) -> Result<String, BadJson> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while !matches!(self.peek(), None | Some(b'"' | b'\\')) {
                if self.text[self.at] < 0x20 {
                    return Err(self.fail("a raw control character inside a string"));
                }
                self.at += 1;
            }
            out.push_str(&self.source[start..self.at]);
            match self.peek() {
                None => return Err(self.fail("a string that never closes")),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                _ => {
                    self.at += 1;
                    let escaped = self.peek().ok_or_else(|| self.fail("a string that never closes"))?;
                    self.at += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode()?),
                        _ => return Err(self.fail("not an escape JSON knows")),
                    }
                }
            }
        }
    }

    fn unicode(&mut self) -> Result<char, BadJson> {
        let first = self.four()?;
        if (0xd800..0xdc00).contains(&first) {
            if !self.text[self.at..].starts_with(b"\\u") {
                return Err(self.fail("half of a surrogate pair"));
            }
            self.at += 2;
            let second = self.four()?;
            if !(0xdc00..0xe000).contains(&second) {
                return Err(self.fail("half of a surrogate pair"));
            }
            let joined = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
            return char::from_u32(joined).ok_or_else(|| self.fail("not a character"));
        }
        char::from_u32(first).ok_or_else(|| self.fail("half of a surrogate pair"))
    }

    fn four(&mut self) -> Result<u32, BadJson> {
        let digits = self.source.get(self.at..self.at + 4).ok_or_else(|| self.fail("\\u needs four hex digits"))?;
        let value = u32::from_str_radix(digits, 16).map_err(|_| self.fail("\\u needs four hex digits"))?;
        self.at += 4;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "tests/json.rs"]
mod tests;
