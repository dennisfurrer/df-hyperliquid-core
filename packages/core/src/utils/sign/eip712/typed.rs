//! EIP-712 typed data as a wallet's `eth_signTypedData_v4` takes it, hashed
//! from the JSON alone: any domain, any struct, structs inside structs, and
//! arrays. [`super::TypedStruct`] builds the venue's own flat structs from
//! typed values; this reads what another program hands over to be signed, so
//! a signer signs the digest of the data it shows and never a digest it is
//! only told.
//!
//! The JSON is read with [`Json`], which keeps keys in their written order
//! and numbers as their digits, so a field list is hashed in the order the
//! data gives it and a `uint256` is never rounded. The types this reads:
//! `bool`, `address`, `string`, `bytes`, `bytes1` to `bytes32`, `uint8` to
//! `uint256` and `int8` to `int256` by eights, structs named in `types`, and
//! arrays of any of them, sized or not.

use crate::utils::json::{BadJson, Json};
use crate::utils::sign::keccak::{Keccak256, keccak256};
use crate::utils::{Address, hex};

/// Why typed data could not be hashed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadTypedData {
    #[error("not JSON: {0}")]
    Json(#[from] BadJson),
    #[error("{0}")]
    Shape(String),
}

fn shape(what: impl Into<String>) -> BadTypedData {
    BadTypedData::Shape(what.into())
}

/// Typed data read for a confirm: its primary type, where it is signed, its
/// message's fields as text, and the digest a signature over it signs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedData {
    pub primary: String,
    /// The domain's fields, in the order the data gives them.
    pub domain: Vec<(String, String)>,
    /// The message's fields, in the order its type lists them.
    pub message: Vec<(String, String)>,
    pub digest: [u8; 32],
}

/// The digest of typed data: `keccak256(0x1901 || domain separator ||
/// hashStruct(message))`.
pub fn digest(text: &str) -> Result<[u8; 32], BadTypedData> {
    Ok(read(text)?.digest)
}

/// Reads typed data, hashes it, and lays its fields out as text.
pub fn read(text: &str) -> Result<TypedData, BadTypedData> {
    let json = Json::parse(text)?;
    let types = json.get("types").and_then(Json::as_object).ok_or_else(|| shape("no `types`"))?;
    let primary = json.get("primaryType").and_then(Json::as_str).ok_or_else(|| shape("no `primaryType`"))?;
    if primary == "EIP712Domain" {
        return Err(shape("the primary type is the domain"));
    }
    let domain = json.get("domain").ok_or_else(|| shape("no `domain`"))?;
    let message = json.get("message").ok_or_else(|| shape("no `message`"))?;
    let types = Types::of(types, domain);
    let separator = types.hash_struct("EIP712Domain", domain, 0)?;
    let hash = types.hash_struct(primary, message, 0)?;
    let digest = Keccak256::new().chain(&[0x19, 0x01]).chain(&separator).chain(&hash).finalize();
    Ok(TypedData {
        primary: primary.to_owned(),
        domain: types.text_fields("EIP712Domain", domain)?,
        message: types.text_fields(primary, message)?,
        digest,
    })
}

/// How deep structs may nest inside one another, so a type that names itself
/// is refused rather than followed for ever.
const DEEPEST: usize = 16;

/// The domain's fields in the order EIP-712 lists them, for data whose
/// `types` leaves `EIP712Domain` out, as a wallet then reads it from the
/// domain's own keys.
const DOMAIN_FIELDS: [(&str, &str); 5] = [
    ("name", "string"),
    ("version", "string"),
    ("chainId", "uint256"),
    ("verifyingContract", "address"),
    ("salt", "bytes32"),
];

/// The `types` object: each struct's name and its fields.
struct Types(Vec<(String, Json)>);

impl Types {
    /// The data's types, with `EIP712Domain` read from the domain's keys when
    /// the data does not list it.
    fn of(types: &[(String, Json)], domain: &Json) -> Types {
        let mut all = types.to_vec();
        if !all.iter().any(|(name, _)| name == "EIP712Domain") {
            let fields = DOMAIN_FIELDS
                .iter()
                .filter(|(name, _)| domain.get(name).is_some())
                .map(|(name, kind)| {
                    Json::Object(vec![
                        ("name".to_owned(), Json::String((*name).to_owned())),
                        ("type".to_owned(), Json::String((*kind).to_owned())),
                    ])
                })
                .collect();
            all.push(("EIP712Domain".to_owned(), Json::Array(fields)));
        }
        Types(all)
    }

    fn fields(&self, name: &str) -> Result<Vec<(&str, &str)>, BadTypedData> {
        let list = self
            .0
            .iter()
            .find(|(key, _)| key == name)
            .and_then(|(_, list)| list.as_array())
            .ok_or_else(|| shape(format!("no type `{name}`")))?;
        list.iter()
            .map(|field| {
                let named = field.get("name").and_then(Json::as_str);
                let typed = field.get("type").and_then(Json::as_str);
                named.zip(typed).ok_or_else(|| shape(format!("a field of `{name}` without a name and a type")))
            })
            .collect()
    }

    fn is_struct(&self, name: &str) -> bool {
        self.0.iter().any(|(key, _)| key == name)
    }

    /// `Name(type field,...)` for `name`, then each struct it reaches, sorted
    /// by name, as EIP-712 encodes a type.
    fn encode_type(&self, name: &str) -> Result<String, BadTypedData> {
        let mut found = Vec::new();
        self.reach(name, &mut found, 0)?;
        found.retain(|one| one != name);
        found.sort();
        let mut out = String::new();
        for one in std::iter::once(name.to_owned()).chain(found) {
            out.push_str(&one);
            out.push('(');
            let fields = self.fields(&one)?;
            for (at, (field, kind)) in fields.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                out.push_str(kind);
                out.push(' ');
                out.push_str(field);
            }
            out.push(')');
        }
        Ok(out)
    }

    fn reach(&self, name: &str, found: &mut Vec<String>, depth: usize) -> Result<(), BadTypedData> {
        if found.iter().any(|one| one == name) {
            return Ok(());
        }
        if depth > DEEPEST {
            return Err(shape("structs nest too deep"));
        }
        found.push(name.to_owned());
        for (_, kind) in self.fields(name)? {
            let base = element(kind);
            if self.is_struct(base) {
                self.reach(base, found, depth + 1)?;
            }
        }
        Ok(())
    }

    fn hash_struct(&self, name: &str, value: &Json, depth: usize) -> Result<[u8; 32], BadTypedData> {
        if depth > DEEPEST {
            return Err(shape("structs nest too deep"));
        }
        let mut sponge = Keccak256::new().chain(&keccak256(self.encode_type(name)?.as_bytes()));
        for (field, kind) in self.fields(name)? {
            let held = value.get(field).ok_or_else(|| shape(format!("`{name}` has no `{field}`")))?;
            sponge = sponge.chain(&self.encode(kind, held, depth)?);
        }
        Ok(sponge.finalize())
    }

    /// One field's word: atomic types as themselves, strings and bytes as
    /// their hash, structs as their struct hash, arrays as the hash of their
    /// elements' words.
    fn encode(&self, kind: &str, value: &Json, depth: usize) -> Result<[u8; 32], BadTypedData> {
        if let Some(open) = kind.rfind('[')
            && kind.ends_with(']')
        {
            let items = value.as_array().ok_or_else(|| shape(format!("a `{kind}` that is not a list")))?;
            let size = &kind[open + 1..kind.len() - 1];
            if !size.is_empty() && size.parse::<usize>().ok() != Some(items.len()) {
                return Err(shape(format!("a `{kind}` of {} items", items.len())));
            }
            let mut sponge = Keccak256::new();
            for item in items {
                sponge = sponge.chain(&self.encode(&kind[..open], item, depth + 1)?);
            }
            return Ok(sponge.finalize());
        }
        if self.is_struct(kind) {
            return self.hash_struct(kind, value, depth + 1);
        }
        atomic(kind, value)
    }

    fn text_fields(&self, name: &str, value: &Json) -> Result<Vec<(String, String)>, BadTypedData> {
        self.fields(name)?
            .into_iter()
            .map(|(field, _)| {
                let held = value.get(field).ok_or_else(|| shape(format!("`{name}` has no `{field}`")))?;
                Ok((field.to_owned(), text(held)))
            })
            .collect()
    }
}

/// An array's element type: `uint256` of `uint256[]` or `uint256[3]`.
fn element(kind: &str) -> &str {
    kind.find('[').map_or(kind, |open| &kind[..open])
}

fn atomic(kind: &str, value: &Json) -> Result<[u8; 32], BadTypedData> {
    match kind {
        "bool" => match value {
            Json::Bool(flag) => Ok(word_of(u8::from(*flag))),
            _ => Err(shape("a `bool` that is not true or false")),
        },
        "address" => {
            let address: Address = value
                .as_str()
                .and_then(|text| text.parse().ok())
                .ok_or_else(|| shape("an `address` that is not 0x and 40 hex"))?;
            let mut word = [0u8; 32];
            word[12..].copy_from_slice(address.bytes());
            Ok(word)
        }
        "string" => {
            let text = value.as_str().ok_or_else(|| shape("a `string` that is not text"))?;
            Ok(keccak256(text.as_bytes()))
        }
        "bytes" => Ok(keccak256(&bytes_of(value, "bytes")?)),
        _ => {
            if let Some(width) = kind.strip_prefix("bytes") {
                let size = bits(width, 1, 32, 1).ok_or_else(|| shape(format!("no type `{kind}`")))?;
                let bytes = bytes_of(value, kind)?;
                if bytes.len() != size {
                    return Err(shape(format!("a `{kind}` of {} bytes", bytes.len())));
                }
                let mut word = [0u8; 32];
                word[..size].copy_from_slice(&bytes);
                return Ok(word);
            }
            if let Some(width) = kind.strip_prefix("uint") {
                let width = bits(width, 8, 256, 8).ok_or_else(|| shape(format!("no type `{kind}`")))?;
                let (negative, word) = number(value, kind)?;
                if negative || !fits(&word, width) {
                    return Err(shape(format!("a `{kind}` out of range")));
                }
                return Ok(word);
            }
            if let Some(width) = kind.strip_prefix("int") {
                let width = bits(width, 8, 256, 8).ok_or_else(|| shape(format!("no type `{kind}`")))?;
                let (negative, magnitude) = number(value, kind)?;
                return signed(negative, magnitude, width).ok_or_else(|| shape(format!("an `{kind}` out of range")));
            }
            Err(shape(format!("no type `{kind}`")))
        }
    }
}

/// A type's width from its digits (`256` of `uint256`), within its bounds and
/// a multiple of `step`.
fn bits(digits: &str, least: usize, most: usize, step: usize) -> Option<usize> {
    let width: usize = digits.parse().ok()?;
    (digits == width.to_string() && (least..=most).contains(&width) && width.is_multiple_of(step)).then_some(width)
}

fn bytes_of(value: &Json, kind: &str) -> Result<Vec<u8>, BadTypedData> {
    value.as_str().and_then(|text| hex::decode(text).ok()).ok_or_else(|| shape(format!("a `{kind}` that is not hex")))
}

fn word_of(small: u8) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[31] = small;
    word
}

/// A number as JSON gives it, a number or a decimal or `0x` string, as its
/// sign and its magnitude in 256 bits.
fn number(value: &Json, kind: &str) -> Result<(bool, [u8; 32]), BadTypedData> {
    let text = match value {
        Json::Number(digits) | Json::String(digits) => digits.as_str(),
        _ => return Err(shape(format!("a `{kind}` that is not a number"))),
    };
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let magnitude = match body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        Some(hex) => from_hex(hex),
        None => from_decimal(body),
    }
    .ok_or_else(|| shape(format!("a `{kind}` that is not a whole number in 256 bits")))?;
    Ok((negative && magnitude != [0u8; 32], magnitude))
}

fn from_decimal(digits: &str) -> Option<[u8; 32]> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut word = [0u8; 32];
    for digit in digits.bytes() {
        let mut carry = u16::from(digit - b'0');
        for byte in word.iter_mut().rev() {
            let next = u16::from(*byte) * 10 + carry;
            *byte = (next & 0xff) as u8;
            carry = next >> 8;
        }
        if carry != 0 {
            return None;
        }
    }
    Some(word)
}

fn from_hex(digits: &str) -> Option<[u8; 32]> {
    if digits.is_empty() || digits.len() > 64 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let padded = format!("{digits:0>64}");
    hex::decode_array::<32>(&padded).ok()
}

/// Whether a magnitude fits in `width` bits.
fn fits(word: &[u8; 32], width: usize) -> bool {
    let free = (256 - width) / 8;
    word[..free].iter().all(|byte| *byte == 0)
}

/// A signed value in `width` bits as its two's-complement word, sign extended.
fn signed(negative: bool, magnitude: [u8; 32], width: usize) -> Option<[u8; 32]> {
    if !negative {
        // at most 2^(width-1) - 1
        let top = 32 - width / 8;
        return (fits(&magnitude, width) && magnitude[top] & 0x80 == 0).then_some(magnitude);
    }
    // at most 2^(width-1)
    let mut limit = [0u8; 32];
    limit[32 - width / 8] = 0x80;
    if magnitude > limit {
        return None;
    }
    // 2^256 - magnitude
    let mut word = [0u8; 32];
    let mut borrow = 0u16;
    for at in (0..32).rev() {
        let take = u16::from(magnitude[at]) + borrow;
        let (byte, next) = match take {
            0 => (0u16, 0u16),
            _ => (256 - take, 1),
        };
        word[at] = byte as u8;
        borrow = next;
    }
    Some(word)
}

/// A field's value as a confirm shows it.
fn text(value: &Json) -> String {
    match value {
        Json::Null => "null".to_owned(),
        Json::Bool(flag) => flag.to_string(),
        Json::Number(digits) | Json::String(digits) => digits.clone(),
        Json::Array(items) => format!("[{}]", items.iter().map(text).collect::<Vec<_>>().join(", ")),
        Json::Object(_) => value.compact(),
    }
}

#[cfg(test)]
#[path = "tests/typed.rs"]
mod tests;
