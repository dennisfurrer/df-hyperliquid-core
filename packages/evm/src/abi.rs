//! Solidity's ABI encoding for the types the chain's read and write surfaces
//! use: integers of every width to 256 bits, addresses, booleans, `bytes1` to
//! `bytes32`, `bytes`, strings, fixed and dynamic arrays, and tuples nested in
//! any of them.
//!
//! Types are read from the human-readable form contracts and docs write them
//! in, `(uint64 bid, uint64 ask)` or `address indexed user, bytes data`, so a
//! declared shape is the one place a call's arguments and its answer are
//! described. A value is checked against its type before it is encoded, and
//! every word is checked against its type as it is decoded. Numbers never pass
//! through a float.

use std::fmt;

use hl_core::{Address, hex, keccak256};

use crate::u256::{B256, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Uint(u16),
    Int(u16),
    Address,
    Bool,
    String,
    Bytes,
    /// `bytes1` to `bytes32`.
    Fixed(usize),
    /// `T[]`.
    Array(Box<Kind>),
    /// `T[n]`.
    Sized(Box<Kind>, usize),
    Tuple(Vec<Param>),
}

/// One parameter: its type, and the name and `indexed` mark it was declared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Option<String>,
    pub kind: Kind,
    pub indexed: bool,
}

/// A value of some [`Kind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Uint(U256),
    /// Two's complement in 256 bits: -1 is all ones, whatever the width.
    Int(U256),
    Address(Address),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    /// The `n` bytes of a `bytesN`.
    Fixed(Vec<u8>),
    /// `T[]` and `T[n]`.
    Array(Vec<Token>),
    Tuple(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AbiError {
    #[error("`{0}` is not a type this reads")]
    Type(String),
    #[error("`{0}` is not a signature this reads; expected name(types)")]
    Signature(String),
    #[error("{name} must be {wants}, and `{got}` is not")]
    Value { name: String, wants: String, got: String },
    #[error("expected {wants} values and there are {got}")]
    Count { wants: usize, got: usize },
    #[error("a {kind} cannot hold {token}")]
    Mismatch { kind: String, token: String },
    #[error("the data ends at byte {0}, before the shape it was declared with")]
    Short(usize),
    #[error("the word at byte {at} does not fit {kind}")]
    Misfit { at: usize, kind: String },
    #[error("the calldata is for selector {found}, not {wants}")]
    Selector { wants: String, found: String },
}

const WORD: usize = 32;

/// The most a length or offset inside the data is believed before it is refused.
const LONGEST: usize = 1 << 20;

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
    fn head(&self) -> usize {
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
    fn wants(&self) -> String {
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

fn is_name(word: &str) -> bool {
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
fn split(text: &str) -> Vec<&str> {
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

/// A function as Solidity or viem writes it: `transfer(address to, uint256
/// amount) returns (bool)`, with or without `function` and its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub inputs: Vec<Param>,
    pub outputs: Vec<Param>,
}

impl Function {
    pub fn parse(text: &str) -> Result<Function, AbiError> {
        let bad = || AbiError::Signature(text.trim().to_owned());
        let (name, inside, rest) = signature_parts(text, "function").ok_or_else(bad)?;
        let inputs = params(inside)?;
        let outputs = match rest.find("returns") {
            Some(at) => {
                let after = rest[at + "returns".len()..].trim();
                let inner = after.strip_prefix('(').and_then(|after| after.strip_suffix(')')).ok_or_else(bad)?;
                params(inner)?
            }
            None => Vec::new(),
        };
        Ok(Function { name: name.to_owned(), inputs, outputs })
    }

    /// `name(t1,t2)`, what the selector hashes.
    #[must_use]
    pub fn signature(&self) -> String {
        format!("{}{}", self.name, tuple_type(&self.inputs))
    }

    /// The four bytes that name the function: the start of `keccak256(signature)`.
    #[must_use]
    pub fn selector(&self) -> [u8; 4] {
        let hash = keccak256(self.signature().as_bytes());
        [hash[0], hash[1], hash[2], hash[3]]
    }

    /// The calldata of a call: the selector, then the arguments.
    pub fn encode(&self, args: &[Token]) -> Result<Vec<u8>, AbiError> {
        let mut out = self.selector().to_vec();
        out.extend(encode(&self.inputs, args)?);
        Ok(out)
    }

    /// A call's arguments back out of its calldata.
    pub fn decode_input(&self, calldata: &[u8]) -> Result<Vec<Token>, AbiError> {
        let found = calldata.get(..4).ok_or(AbiError::Short(calldata.len()))?;
        if found != self.selector() {
            return Err(AbiError::Selector {
                wants: hex::encode_prefixed(&self.selector()),
                found: hex::encode_prefixed(found),
            });
        }
        decode(&self.inputs, &calldata[4..])
    }

    /// What the call answered, read against its declared outputs.
    pub fn decode_output(&self, answer: &[u8]) -> Result<Vec<Token>, AbiError> {
        decode(&self.outputs, answer)
    }
}

/// `name`, what is inside its parentheses, and what follows them.
fn signature_parts<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let text = text.trim();
    let text = text.strip_prefix(keyword).filter(|rest| rest.starts_with(char::is_whitespace)).unwrap_or(text).trim();
    let open = text.find('(')?;
    let name = text[..open].trim();
    if name.is_empty() || !is_name(name) {
        return None;
    }
    let mut depth = 0i32;
    for (at, letter) in text[open..].char_indices() {
        match letter {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let close = open + at;
                    return Some((name, &text[open + 1..close], &text[close + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// The selector of a function signature in any spelling the docs use.
pub fn selector(signature: &str) -> Result<[u8; 4], AbiError> {
    Function::parse(signature).map(|function| function.selector())
}

/// The first topic an event is logged under: `keccak256(name(t1,t2))`.
pub fn event_topic(signature: &str) -> Result<B256, AbiError> {
    let (name, inside, _) =
        signature_parts(signature, "event").ok_or_else(|| AbiError::Signature(signature.trim().to_owned()))?;
    let canonical = format!("{name}{}", tuple_type(&params(inside)?));
    Ok(B256(keccak256(canonical.as_bytes())))
}

/// `abi.encode(values...)`: heads, then the tails they point into. Each value
/// is checked against its type first.
pub fn encode(params: &[Param], tokens: &[Token]) -> Result<Vec<u8>, AbiError> {
    if params.len() != tokens.len() {
        return Err(AbiError::Count { wants: params.len(), got: tokens.len() });
    }
    for (param, token) in params.iter().zip(tokens) {
        if !fits(&param.kind, token) {
            return Err(AbiError::Mismatch { kind: param.kind.canonical(), token: token.describe() });
        }
    }
    let kinds: Vec<&Kind> = params.iter().map(|param| &param.kind).collect();
    Ok(sequence(&kinds, tokens))
}

/// Whether `token` is a value of `kind`, its integers within their width.
fn fits(kind: &Kind, token: &Token) -> bool {
    match (kind, token) {
        (Kind::Uint(bits), Token::Uint(value)) => value.bits() <= u32::from(*bits),
        (Kind::Int(bits), Token::Int(value)) => sign_extended(&value.to_be_bytes(), *bits),
        (Kind::Address, Token::Address(_))
        | (Kind::Bool, Token::Bool(_))
        | (Kind::String, Token::String(_))
        | (Kind::Bytes, Token::Bytes(_)) => true,
        (Kind::Fixed(size), Token::Fixed(bytes)) => bytes.len() == *size,
        (Kind::Array(inner), Token::Array(items)) => items.iter().all(|item| fits(inner, item)),
        (Kind::Sized(inner, count), Token::Array(items)) => {
            items.len() == *count && items.iter().all(|item| fits(inner, item))
        }
        (Kind::Tuple(params), Token::Tuple(items)) => {
            params.len() == items.len() && params.iter().zip(items).all(|(param, item)| fits(&param.kind, item))
        }
        _ => false,
    }
}

/// Whether a two's complement word is the sign extension of its low `bits`.
fn sign_extended(word: &[u8; WORD], bits: u16) -> bool {
    let spare = WORD - usize::from(bits) / 8;
    if spare == 0 {
        return true;
    }
    let fill = if word[spare] & 0x80 == 0 { 0x00 } else { 0xff };
    word[..spare].iter().all(|byte| *byte == fill)
}

fn sequence(kinds: &[&Kind], tokens: &[Token]) -> Vec<u8> {
    let heads: usize = kinds.iter().map(|kind| kind.head()).sum();
    let mut head = Vec::with_capacity(heads);
    let mut tail = Vec::new();
    for (kind, token) in kinds.iter().zip(tokens) {
        let encoded = one(kind, token);
        match kind.is_dynamic() {
            true => {
                head.extend_from_slice(&word_of(heads + tail.len()));
                tail.extend(encoded);
            }
            false => head.extend(encoded),
        }
    }
    head.extend(tail);
    head
}

/// One value checked by [`fits`]: its kind and token agree.
fn one(kind: &Kind, token: &Token) -> Vec<u8> {
    match (kind, token) {
        (_, Token::Uint(value) | Token::Int(value)) => value.to_be_bytes().to_vec(),
        (_, Token::Address(address)) => B256::from_address(*address).0.to_vec(),
        (_, Token::Bool(value)) => word_of(usize::from(*value)).to_vec(),
        (_, Token::String(text)) => packed(text.as_bytes()),
        (_, Token::Bytes(bytes)) => packed(bytes),
        (_, Token::Fixed(bytes)) => padded(bytes),
        (Kind::Array(inner), Token::Array(items)) => {
            let mut out = word_of(items.len()).to_vec();
            out.extend(sequence(&vec![inner.as_ref(); items.len()], items));
            out
        }
        (Kind::Sized(inner, _), Token::Array(items)) => sequence(&vec![inner.as_ref(); items.len()], items),
        (Kind::Tuple(params), Token::Tuple(items)) => {
            let kinds: Vec<&Kind> = params.iter().map(|param| &param.kind).collect();
            sequence(&kinds, items)
        }
        (_, Token::Array(items) | Token::Tuple(items)) => sequence(&vec![kind; items.len()], items),
    }
}

fn packed(bytes: &[u8]) -> Vec<u8> {
    let mut out = word_of(bytes.len()).to_vec();
    out.extend(padded(bytes));
    out
}

fn padded(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out.resize(bytes.len().div_ceil(WORD) * WORD, 0);
    out
}

fn word_of(value: usize) -> [u8; WORD] {
    U256::from_u64(value as u64).to_be_bytes()
}

/// `abi.decode(data, (types...))`, each word checked against its declared width.
pub fn decode(params: &[Param], data: &[u8]) -> Result<Vec<Token>, AbiError> {
    let kinds: Vec<&Kind> = params.iter().map(|param| &param.kind).collect();
    unsequence(&kinds, data, 0)
}

fn unsequence(kinds: &[&Kind], data: &[u8], start: usize) -> Result<Vec<Token>, AbiError> {
    let mut at = start;
    let mut tokens = Vec::with_capacity(kinds.len().min(LONGEST / WORD));
    for kind in kinds {
        match kind.is_dynamic() {
            true => {
                let offset = length(data, at)?;
                tokens.push(unone(kind, data, start + offset)?);
                at += WORD;
            }
            false => {
                tokens.push(unone(kind, data, at)?);
                at += kind.head();
            }
        }
    }
    Ok(tokens)
}

fn word(data: &[u8], at: usize) -> Result<[u8; WORD], AbiError> {
    data.get(at..at.saturating_add(WORD)).and_then(|slice| slice.try_into().ok()).ok_or(AbiError::Short(data.len()))
}

/// A length or an offset: small, or refused.
fn length(data: &[u8], at: usize) -> Result<usize, AbiError> {
    let value = U256::from_be_bytes(word(data, at)?);
    value
        .to_u64()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value <= LONGEST)
        .ok_or_else(|| AbiError::Misfit { at, kind: "a length".to_owned() })
}

fn unone(kind: &Kind, data: &[u8], at: usize) -> Result<Token, AbiError> {
    let misfit = || AbiError::Misfit { at, kind: kind.canonical() };
    Ok(match kind {
        Kind::Uint(bits) => {
            let value = U256::from_be_bytes(word(data, at)?);
            if value.bits() > u32::from(*bits) {
                return Err(misfit());
            }
            Token::Uint(value)
        }
        Kind::Int(bits) => {
            let word = word(data, at)?;
            if !sign_extended(&word, *bits) {
                return Err(misfit());
            }
            Token::Int(U256::from_be_bytes(word))
        }
        Kind::Address => B256(word(data, at)?).to_address().map(Token::Address).ok_or_else(misfit)?,
        Kind::Bool => match U256::from_be_bytes(word(data, at)?).to_u64() {
            Some(0) => Token::Bool(false),
            Some(1) => Token::Bool(true),
            _ => return Err(misfit()),
        },
        Kind::Fixed(size) => {
            let word = word(data, at)?;
            if word[*size..].iter().any(|byte| *byte != 0) {
                return Err(misfit());
            }
            Token::Fixed(word[..*size].to_vec())
        }
        Kind::String | Kind::Bytes => {
            let length = length(data, at)?;
            let start = at + WORD;
            let bytes = data.get(start..start + length).ok_or(AbiError::Short(data.len()))?.to_vec();
            match kind {
                Kind::String => Token::String(String::from_utf8(bytes).map_err(|_| misfit())?),
                _ => Token::Bytes(bytes),
            }
        }
        Kind::Array(inner) => {
            let count = length(data, at)?;
            if count.saturating_mul(inner.head().max(1)) > data.len() {
                return Err(AbiError::Short(data.len()));
            }
            Token::Array(unsequence(&vec![inner.as_ref(); count], data, at + WORD)?)
        }
        Kind::Sized(inner, count) => {
            if count.saturating_mul(inner.head().max(1)) > data.len() {
                return Err(AbiError::Short(data.len()));
            }
            Token::Array(unsequence(&vec![inner.as_ref(); *count], data, at)?)
        }
        Kind::Tuple(params) => {
            let kinds: Vec<&Kind> = params.iter().map(|param| &param.kind).collect();
            Token::Tuple(unsequence(&kinds, data, at)?)
        }
    })
}

impl Token {
    #[must_use]
    pub fn uint(value: impl Into<U256>) -> Token {
        Token::Uint(value.into())
    }

    #[must_use]
    pub fn int(value: i128) -> Token {
        let magnitude = U256::from_u128(value.unsigned_abs());
        Token::Int(match value < 0 {
            true => negated(magnitude),
            false => magnitude,
        })
    }

    #[must_use]
    pub fn address(address: Address) -> Token {
        Token::Address(address)
    }

    /// A typed value read into this kind: decimal for integers, `0x` hex for
    /// addresses and bytes, `true` or `false`, and `[a, b]` or `(a, b)` for
    /// the containers. `name` is what a refusal calls the field.
    pub fn parse(kind: &Kind, name: &str, text: &str) -> Result<Token, AbiError> {
        let text = text.trim();
        let refused = || AbiError::Value { name: name.to_owned(), wants: kind.wants(), got: text.to_owned() };
        Ok(match kind {
            Kind::Uint(bits) => {
                let value = U256::from_decimal(text).map_err(|_| refused())?;
                if value.bits() > u32::from(*bits) {
                    return Err(refused());
                }
                Token::Uint(value)
            }
            Kind::Int(bits) => {
                let (negative, digits) = match text.strip_prefix('-') {
                    Some(rest) => (true, rest),
                    None => (false, text),
                };
                let magnitude = U256::from_decimal(digits).map_err(|_| refused())?;
                // One bit is the sign: a magnitude below 2^(bits-1), or exactly
                // that when it is negative.
                let room = u32::from(*bits) - 1;
                let edge = magnitude.bits() == room + 1 && magnitude == power_of_two(room);
                if magnitude.bits() > room && !(negative && edge) {
                    return Err(refused());
                }
                Token::Int(match negative {
                    true => negated(magnitude),
                    false => magnitude,
                })
            }
            Kind::Address => Token::Address(text.parse().map_err(|_| refused())?),
            Kind::Bool => match text.to_ascii_lowercase().as_str() {
                "true" => Token::Bool(true),
                "false" => Token::Bool(false),
                _ => return Err(refused()),
            },
            Kind::String => Token::String(text.to_owned()),
            Kind::Bytes => Token::Bytes(hex_of(text).ok_or_else(refused)?),
            Kind::Fixed(size) => Token::Fixed(hex_of(text).filter(|bytes| bytes.len() == *size).ok_or_else(refused)?),
            Kind::Array(inner) | Kind::Sized(inner, _) => {
                let inside = text.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).ok_or_else(refused)?;
                let items: Vec<&str> = match inside.trim().is_empty() {
                    true => Vec::new(),
                    false => split(inside),
                };
                if let Kind::Sized(_, count) = kind
                    && items.len() != *count
                {
                    return Err(refused());
                }
                Token::Array(items.iter().map(|item| Token::parse(inner, name, item)).collect::<Result<_, _>>()?)
            }
            Kind::Tuple(params) => {
                let inside = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')).ok_or_else(refused)?;
                let items = split(inside);
                if items.len() != params.len() {
                    return Err(refused());
                }
                Token::Tuple(
                    params
                        .iter()
                        .zip(items)
                        .map(|(param, item)| Token::parse(&param.kind, param.name.as_deref().unwrap_or(name), item))
                        .collect::<Result<_, _>>()?,
                )
            }
        })
    }

    /// The value as a person reads it: decimal integers, `0x` hex, and the
    /// containers in brackets, a tuple's fields named where `kind` names them.
    #[must_use]
    pub fn show(&self, kind: &Kind) -> String {
        match (self, kind) {
            (Token::Uint(value), _) => value.to_string(),
            (Token::Int(value), _) => signed(*value),
            (Token::Address(address), _) => address.to_hex(),
            (Token::Bool(value), _) => value.to_string(),
            (Token::String(text), _) => text.clone(),
            (Token::Bytes(bytes) | Token::Fixed(bytes), _) => hex::encode_prefixed(bytes),
            (Token::Array(items), Kind::Array(inner) | Kind::Sized(inner, _)) => {
                format!("[{}]", items.iter().map(|item| item.show(inner)).collect::<Vec<_>>().join(", "))
            }
            (Token::Tuple(items), Kind::Tuple(params)) => format!(
                "{{{}}}",
                items
                    .iter()
                    .zip(params)
                    .map(|(item, param)| match &param.name {
                        Some(name) => format!("{name}: {}", item.show(&param.kind)),
                        None => item.show(&param.kind),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            (Token::Array(items) | Token::Tuple(items), _) => {
                format!("[{}]", items.iter().map(|item| item.show(kind)).collect::<Vec<_>>().join(", "))
            }
        }
    }

    /// A short name of the value, for a refusal.
    fn describe(&self) -> String {
        match self {
            Token::Uint(value) => format!("the number {value}"),
            Token::Int(value) => format!("the number {}", signed(*value)),
            Token::Address(address) => format!("the address {address}"),
            Token::Bool(value) => format!("{value}"),
            Token::String(_) => "a string".to_owned(),
            Token::Bytes(bytes) => format!("{} bytes", bytes.len()),
            Token::Fixed(bytes) => format!("{} fixed bytes", bytes.len()),
            Token::Array(items) => format!("a list of {}", items.len()),
            Token::Tuple(items) => format!("a tuple of {}", items.len()),
        }
    }

    #[must_use]
    pub fn as_u256(&self) -> Option<U256> {
        match self {
            Token::Uint(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_u128(&self) -> Option<u128> {
        self.as_u256().and_then(U256::to_u128)
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.as_u256().and_then(U256::to_u64)
    }

    #[must_use]
    pub fn as_u32(&self) -> Option<u32> {
        self.as_u64().and_then(|value| u32::try_from(value).ok())
    }

    #[must_use]
    pub fn as_u16(&self) -> Option<u16> {
        self.as_u64().and_then(|value| u16::try_from(value).ok())
    }

    #[must_use]
    pub fn as_u8(&self) -> Option<u8> {
        self.as_u64().and_then(|value| u8::try_from(value).ok())
    }

    /// A signed integer, while it fits an `i128`.
    #[must_use]
    pub fn as_i128(&self) -> Option<i128> {
        let Token::Int(value) = self else { return None };
        match value.bits() == 256 {
            false => value.to_u128().and_then(|value| i128::try_from(value).ok()),
            true => {
                let magnitude = negated(*value).to_u128()?;
                match magnitude == i128::MIN.unsigned_abs() {
                    true => Some(i128::MIN),
                    false => i128::try_from(magnitude).ok().map(|magnitude| -magnitude),
                }
            }
        }
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.as_i128().and_then(|value| i64::try_from(value).ok())
    }

    #[must_use]
    pub fn as_i8(&self) -> Option<i8> {
        self.as_i128().and_then(|value| i8::try_from(value).ok())
    }

    #[must_use]
    pub fn as_address(&self) -> Option<Address> {
        match self {
            Token::Address(address) => Some(*address),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Token::Bool(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Token::String(text) => Some(text),
            _ => None,
        }
    }

    /// The bytes of a `bytes` or a `bytesN`.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Token::Bytes(bytes) | Token::Fixed(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The items of an array or the fields of a tuple.
    #[must_use]
    pub fn as_items(&self) -> Option<&[Token]> {
        match self {
            Token::Array(items) | Token::Tuple(items) => Some(items),
            _ => None,
        }
    }
}

/// `-value` in two's complement: `2^256 - value`, and zero for zero.
fn negated(value: U256) -> U256 {
    match value.checked_sub(U256::ONE) {
        Some(less) => U256::MAX.checked_sub(less).unwrap_or(U256::ZERO),
        None => U256::ZERO,
    }
}

fn power_of_two(exponent: u32) -> U256 {
    let mut word = [0u8; WORD];
    let at = WORD - 1 - (exponent / 8) as usize;
    word[at] = 1 << (exponent % 8);
    U256::from_be_bytes(word)
}

/// A two's complement word in decimal, with its sign.
fn signed(value: U256) -> String {
    match value.bits() == 256 {
        true => format!("-{}", negated(value)),
        false => value.to_string(),
    }
}

fn hex_of(text: &str) -> Option<Vec<u8>> {
    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))?;
    hex::decode(text).ok()
}

#[cfg(test)]
#[path = "tests/abi.rs"]
mod tests;
