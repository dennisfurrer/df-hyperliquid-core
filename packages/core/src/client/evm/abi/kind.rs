//! The type grammar: a [`Kind`] and the [`Param`]s it is declared in, read from
//! their human-readable form and written back canonically.

use std::fmt;

use crate::client::evm::abi::{AbiError, LONGEST, WORD};

/// A Solidity ABI type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `uintN`, with N the width in bits: 8 to 256 in steps of 8. `uint` reads as `uint256`.
    Uint(u16),
    /// `intN`, with N the width in bits: 8 to 256 in steps of 8. `int` reads as `int256`.
    Int(u16),
    /// `address`, 20 bytes.
    Address,
    /// `bool`.
    Bool,
    /// `string`, UTF-8 of any length.
    String,
    /// `bytes`, of any length.
    Bytes,
    /// `bytes1` to `bytes32`.
    Fixed(usize),
    /// `T[]`.
    Array(Box<Kind>),
    /// `T[n]`.
    Sized(Box<Kind>, usize),
    /// A tuple, written `(T1, T2)`.
    Tuple(Vec<Param>),
}

/// One parameter: its type, and the name and `indexed` mark it was declared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name it was declared with, or `None` when it has none.
    pub name: Option<String>,
    /// Its type.
    pub kind: Kind,
    /// Whether it was declared `indexed`, as an event parameter can be.
    pub indexed: bool,
}

/// How deep types may nest in a written signature.
const DEEPEST: usize = 32;

/// Words a declaration may carry beside its type and name.
const MODIFIERS: [&str; 4] = ["memory", "calldata", "storage", "payable"];

impl Kind {
    /// One type, as written: `uint`, `bytes32`, `(address, uint64)[]`.
    pub fn parse(text: &str) -> Result<Kind, AbiError> {
        kind_of(text, 0).ok_or_else(|| AbiError::Type(text.trim().to_owned()))
    }

    /// Whether it is encoded behind an offset, in the tail.
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        match self {
            Kind::String | Kind::Bytes | Kind::Array(_) => true,
            Kind::Sized(inner, _) => inner.is_dynamic(),
            Kind::Tuple(params) => params.iter().any(|param| param.kind.is_dynamic()),
            Kind::Uint(_) | Kind::Int(_) | Kind::Address | Kind::Bool | Kind::Fixed(_) => false,
        }
    }

    /// Bytes this takes in the head of whatever holds it.
    pub(super) fn head(&self) -> usize {
        if self.is_dynamic() {
            return WORD;
        }
        match self {
            Kind::Sized(inner, count) => inner.head().saturating_mul(*count),
            Kind::Tuple(params) => params.iter().map(|param| param.kind.head()).sum(),
            _ => WORD,
        }
    }

    /// The canonical type, as a signature or a selector spells it.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Kind::Uint(bits) => format!("uint{bits}"),
            Kind::Int(bits) => format!("int{bits}"),
            Kind::Address => "address".to_owned(),
            Kind::Bool => "bool".to_owned(),
            Kind::String => "string".to_owned(),
            Kind::Bytes => "bytes".to_owned(),
            Kind::Fixed(size) => format!("bytes{size}"),
            Kind::Array(inner) => format!("{}[]", inner.canonical()),
            Kind::Sized(inner, count) => format!("{}[{count}]", inner.canonical()),
            Kind::Tuple(params) => tuple_type(params),
        }
    }

    /// What a value of this kind looks like typed, for a refusal to name.
    pub(super) fn wants(&self) -> String {
        match self {
            Kind::Uint(bits) => format!("a whole number that fits uint{bits}"),
            Kind::Int(bits) => format!("a whole number that fits int{bits}"),
            Kind::Address => "a 20-byte address".to_owned(),
            Kind::Bool => "true or false".to_owned(),
            Kind::String => "text".to_owned(),
            Kind::Bytes => "hex bytes".to_owned(),
            Kind::Fixed(size) => format!("{size} hex bytes"),
            Kind::Array(_) | Kind::Sized(..) => "a list in brackets".to_owned(),
            Kind::Tuple(_) => "a tuple in parentheses".to_owned(),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

/// `(t1,t2,...)`: the canonical spelling of a parameter list.
#[must_use]
pub fn tuple_type(params: &[Param]) -> String {
    let kinds: Vec<String> = params.iter().map(|param| param.kind.canonical()).collect();
    format!("({})", kinds.join(","))
}

/// Reads a parameter list: `uint64, bool`, or `(uint64 bid, uint64 ask)`,
/// which is one tuple, as viem's `parseAbiParameters` reads it.
pub fn params(text: &str) -> Result<Vec<Param>, AbiError> {
    params_at(text, 0).ok_or_else(|| AbiError::Type(text.trim().to_owned()))
}

fn params_at(text: &str, depth: usize) -> Option<Vec<Param>> {
    let text = text.trim();
    if text.is_empty() {
        return Some(Vec::new());
    }
    split(text).into_iter().map(|one| param(one.trim(), depth)).collect()
}

/// A type, then any of `indexed` and the storage words, then a name.
fn param(text: &str, depth: usize) -> Option<Param> {
    let end = type_end(text);
    let kind = kind_of(&text[..end], depth)?;
    let mut name = None;
    let mut indexed = false;
    for word in text[end..].split_whitespace() {
        match word {
            "indexed" => indexed = true,
            _ if MODIFIERS.contains(&word) => {}
            _ if name.is_none() && is_name(word) => name = Some(word.to_owned()),
            _ => return None,
        }
    }
    Some(Param { name, kind, indexed })
}

pub(super) fn is_name(word: &str) -> bool {
    word.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '_' || letter == '$')
}

/// Where the type ends: the first space outside brackets and parentheses.
fn type_end(text: &str) -> usize {
    let mut depth = 0i32;
    for (at, letter) in text.char_indices() {
        match letter {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ if letter.is_whitespace() && depth == 0 => return at,
            _ => {}
        }
    }
    text.len()
}

fn depth_at(text: &str, at: usize) -> i32 {
    text[..at].chars().fold(0, |depth, letter| match letter {
        '(' | '[' => depth + 1,
        ')' | ']' => depth - 1,
        _ => depth,
    })
}

fn kind_of(text: &str, depth: usize) -> Option<Kind> {
    if depth > DEEPEST {
        return None;
    }
    let text = text.trim();
    if let Some(open) = text.rfind('[')
        && text.ends_with(']')
        && depth_at(text, open) == 0
    {
        let inner = kind_of(&text[..open], depth + 1)?;
        let count = &text[open + 1..text.len() - 1];
        return Some(match count.is_empty() {
            true => Kind::Array(Box::new(inner)),
            false => Kind::Sized(Box::new(inner), count.parse().ok().filter(|count| *count <= LONGEST)?),
        });
    }
    if let Some(inside) = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        return params_at(inside, depth + 1).map(Kind::Tuple);
    }
    let bits = |digits: &str| -> Option<u16> {
        match digits.is_empty() {
            true => Some(256),
            false => digits.parse().ok().filter(|bits| *bits > 0 && *bits <= 256 && bits % 8 == 0),
        }
    };
    Some(match text {
        "address" => Kind::Address,
        "bool" => Kind::Bool,
        "string" => Kind::String,
        "bytes" => Kind::Bytes,
        _ if text.starts_with("uint") => Kind::Uint(bits(&text[4..])?),
        _ if text.starts_with("int") => Kind::Int(bits(&text[3..])?),
        _ if text.starts_with("bytes") => {
            let size: usize = text[5..].parse().ok()?;
            (1..=32).contains(&size).then_some(Kind::Fixed(size))?
        }
        _ => return None,
    })
}

/// Splits at the commas that are not inside brackets or parentheses.
pub(super) fn split(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (at, letter) in text.char_indices() {
        match letter {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

#[cfg(test)]
#[path = "tests/kind.rs"]
mod tests;
