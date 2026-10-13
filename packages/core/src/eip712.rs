//! EIP-712 for the flat structs Hyperliquid signs, and HyperEVM contracts that
//! take the same kind (one level, `uint256[]` the only array): hashing, and the
//! typed data a browser wallet's `eth_signTypedData_v4` is handed.

use crate::{
    address::Address,
    hex,
    json::{BadJson, Json},
    keccak::{Keccak256, keccak256},
    msgpack::Value,
    network::{L1_CHAIN_ID, Network},
};

pub const DOMAIN_TYPE: &str = "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    pub name: &'static str,
    pub version: &'static str,
    pub chain_id: u64,
    pub verifying_contract: Address,
}

impl Domain {
    /// What an L1 action's phantom agent is signed under, on both networks.
    /// The chain id is the literal 1337, whatever chain the wallet is on; the
    /// network is in the phantom agent's `source`.
    pub const EXCHANGE: Domain =
        Domain { name: "Exchange", version: "1", chain_id: L1_CHAIN_ID, verifying_contract: Address::ZERO };

    /// What a user-signed action is signed under: the chain the action names
    /// as its `signatureChainId`.
    #[must_use]
    pub const fn user_signed(network: Network) -> Domain {
        Domain {
            name: "HyperliquidSignTransaction",
            version: "1",
            chain_id: network.signature_chain_id(),
            verifying_contract: Address::ZERO,
        }
    }

    #[must_use]
    pub fn separator(&self) -> [u8; 32] {
        Keccak256::new()
            .chain(&keccak256(DOMAIN_TYPE.as_bytes()))
            .chain(&keccak256(self.name.as_bytes()))
            .chain(&keccak256(self.version.as_bytes()))
            .chain(&word(self.chain_id))
            .chain(&address_word(self.verifying_contract))
            .finalize()
    }

    /// The `domain` object of wallet typed data: `chainId` a number, as viem sends it.
    fn to_json(self) -> Json {
        Json::Object(vec![
            ("name".to_owned(), Json::String(self.name.to_owned())),
            ("version".to_owned(), Json::String(self.version.to_owned())),
            ("chainId".to_owned(), Json::Number(self.chain_id.to_string())),
            ("verifyingContract".to_owned(), Json::String(self.verifying_contract.to_hex())),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field<'a> {
    String(&'a str),
    Address(Address),
    Uint64(u64),
    Bytes32([u8; 32]),
    Bool(bool),
    Uint32(u32),
    Uint128(u128),
    /// The big-endian word; [`uint256`] makes one from a smaller number.
    Uint256([u8; 32]),
    /// Each element's big-endian word, in order.
    Uint256Array(&'a [[u8; 32]]),
}

/// The `uint256` word of a number that fits 128 bits.
#[must_use]
pub const fn uint256(value: u128) -> [u8; 32] {
    let bytes = value.to_be_bytes();
    let mut word = [0u8; 32];
    let mut index = 0;
    while index < 16 {
        word[16 + index] = bytes[index];
        index += 1;
    }
    word
}

impl Field<'_> {
    #[must_use]
    pub const fn solidity_type(&self) -> &'static str {
        match self {
            Field::String(_) => "string",
            Field::Address(_) => "address",
            Field::Uint64(_) => "uint64",
            Field::Bytes32(_) => "bytes32",
            Field::Bool(_) => "bool",
            Field::Uint32(_) => "uint32",
            Field::Uint128(_) => "uint128",
            Field::Uint256(_) => "uint256",
            Field::Uint256Array(_) => "uint256[]",
        }
    }

    /// The field's word in the struct hash. An array's is the keccak of its
    /// elements' words, as EIP-712 encodes one.
    #[must_use]
    pub fn encode(&self) -> [u8; 32] {
        match self {
            Field::String(text) => keccak256(text.as_bytes()),
            Field::Address(address) => address_word(*address),
            Field::Uint64(number) => word(*number),
            Field::Bytes32(bytes) | Field::Uint256(bytes) => *bytes,
            Field::Bool(value) => word(u64::from(*value)),
            Field::Uint32(number) => word(u64::from(*number)),
            Field::Uint128(number) => uint256(*number),
            Field::Uint256Array(words) => {
                words.iter().fold(Keccak256::new(), |sponge, element| sponge.chain(element)).finalize()
            }
        }
    }

    /// The value as a wallet's `message` carries it: numbers as JSON numbers,
    /// as viem passes them, bools as JSON bools, the rest as strings. A
    /// `uint128` or `uint256` is a decimal string, as viem sends a bigint:
    /// JavaScript reads a JSON number past 2^53 inexactly.
    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Field::String(text) => Json::String((*text).to_owned()),
            Field::Address(address) => Json::String(address.to_hex()),
            Field::Uint64(number) => Json::Number(number.to_string()),
            Field::Bytes32(bytes) => Json::String(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Json::Bool(*value),
            Field::Uint32(number) => Json::Number(number.to_string()),
            Field::Uint128(number) => Json::String(number.to_string()),
            Field::Uint256(bytes) => Json::String(word_decimal(bytes)),
            Field::Uint256Array(words) => {
                Json::Array(words.iter().map(|element| Json::String(word_decimal(element))).collect())
            }
        }
    }

    /// The value as the posted action carries it: addresses lowercase. A
    /// number past 64 bits is its decimal string, as no msgpack integer holds it.
    #[must_use]
    pub fn to_wire(&self) -> Value {
        match self {
            Field::String(text) => Value::from((*text).to_owned()),
            Field::Address(address) => Value::from(address.to_hex()),
            Field::Uint64(number) => Value::Uint(*number),
            Field::Bytes32(bytes) => Value::from(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Value::Bool(*value),
            Field::Uint32(number) => Value::Uint(u64::from(*number)),
            Field::Uint128(number) => Value::from(number.to_string()),
            Field::Uint256(bytes) => Value::from(word_decimal(bytes)),
            Field::Uint256Array(words) => {
                Value::Array(words.iter().map(|element| Value::from(word_decimal(element))).collect())
            }
        }
    }
}

/// A big-endian word as its decimal digits.
#[must_use]
pub fn word_decimal(bytes: &[u8; 32]) -> String {
    // Nineteen digits at a time: 10^19 is the largest power of ten under 2^64.
    const GROUP: u128 = 10_000_000_000_000_000_000;
    let mut limbs = [0u64; 4];
    for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks_exact(8)) {
        let mut eight = [0u8; 8];
        eight.copy_from_slice(chunk);
        *limb = u64::from_be_bytes(eight);
    }
    let mut groups = Vec::new();
    while limbs.iter().any(|limb| *limb != 0) {
        let mut remainder = 0u128;
        for limb in &mut limbs {
            let value = (remainder << 64) | u128::from(*limb);
            // The remainder is under 10^19, so the quotient is under 2^64.
            *limb = (value / GROUP) as u64;
            remainder = value % GROUP;
        }
        groups.push(remainder as u64);
    }
    let Some((first, rest)) = groups.split_last() else {
        return "0".to_owned();
    };
    let mut text = first.to_string();
    for group in rest.iter().rev() {
        text.push_str(&format!("{group:019}"));
    }
    text
}

/// Decimal digits as a big-endian word; `None` for anything else or past 2^256.
#[must_use]
pub fn decimal_word(digits: &str) -> Option<[u8; 32]> {
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut limbs = [0u64; 4];
    for byte in digits.bytes() {
        let mut carry = u128::from(byte - b'0');
        for limb in limbs.iter_mut().rev() {
            let value = u128::from(*limb) * 10 + carry;
            // The low 64 bits stay in the limb; the rest carries up.
            *limb = value as u64;
            carry = value >> 64;
        }
        if carry != 0 {
            return None;
        }
    }
    let mut word = [0u8; 32];
    for (chunk, limb) in word.chunks_exact_mut(8).zip(limbs) {
        chunk.copy_from_slice(&limb.to_be_bytes());
    }
    Some(word)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStruct<'a> {
    primary_type: &'a str,
    fields: Vec<(&'a str, Field<'a>)>,
}

impl<'a> TypedStruct<'a> {
    #[must_use]
    pub const fn new(primary_type: &'a str) -> Self {
        TypedStruct { primary_type, fields: Vec::new() }
    }

    /// Appends a field; call order is the order in the type string and hash.
    #[must_use]
    pub fn field(mut self, name: &'a str, value: Field<'a>) -> Self {
        self.fields.push((name, value));
        self
    }

    #[must_use]
    pub const fn primary_type(&self) -> &'a str {
        self.primary_type
    }

    #[must_use]
    pub fn fields(&self) -> &[(&'a str, Field<'a>)] {
        &self.fields
    }

    #[must_use]
    pub fn type_signature(&self) -> String {
        let mut text = String::with_capacity(self.primary_type.len() + 32);
        text.push_str(self.primary_type);
        text.push('(');
        for (index, (name, value)) in self.fields.iter().enumerate() {
            if index > 0 {
                text.push(',');
            }
            text.push_str(value.solidity_type());
            text.push(' ');
            text.push_str(name);
        }
        text.push(')');
        text
    }

    #[must_use]
    pub fn type_hash(&self) -> [u8; 32] {
        keccak256(self.type_signature().as_bytes())
    }

    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        let mut sponge = Keccak256::new().chain(&self.type_hash());
        for (_, value) in &self.fields {
            sponge = sponge.chain(&value.encode());
        }
        sponge.finalize()
    }

    /// What the key signs under `domain`.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> [u8; 32] {
        digest(domain, self)
    }

    /// The typed data a browser wallet's `eth_signTypedData_v4` takes, as one
    /// line: `{types: {EIP712Domain, <primary>}, primaryType, domain, message}`.
    /// A wallet that signs it signs [`TypedStruct::digest`].
    #[must_use]
    pub fn wallet_json(&self, domain: &Domain) -> String {
        let list = |pairs: Vec<(&str, &str)>| {
            Json::Array(
                pairs
                    .into_iter()
                    .map(|(name, kind)| {
                        Json::Object(vec![
                            ("name".to_owned(), Json::String(name.to_owned())),
                            ("type".to_owned(), Json::String(kind.to_owned())),
                        ])
                    })
                    .collect(),
            )
        };
        let domain_fields = list(vec![
            ("name", "string"),
            ("version", "string"),
            ("chainId", "uint256"),
            ("verifyingContract", "address"),
        ]);
        let struct_fields = list(self.fields.iter().map(|(name, value)| (*name, value.solidity_type())).collect());
        let message =
            Json::Object(self.fields.iter().map(|(name, value)| ((*name).to_owned(), value.to_json())).collect());
        Json::Object(vec![
            (
                "types".to_owned(),
                Json::Object(vec![
                    ("EIP712Domain".to_owned(), domain_fields),
                    (self.primary_type.to_owned(), struct_fields),
                ]),
            ),
            ("primaryType".to_owned(), Json::String(self.primary_type.to_owned())),
            ("domain".to_owned(), domain.to_json()),
            ("message".to_owned(), message),
        ])
        .compact()
    }
}

/// `keccak256(0x1901 || domain separator || struct hash)`.
#[must_use]
pub fn digest(domain: &Domain, data: &TypedStruct<'_>) -> [u8; 32] {
    Keccak256::new().chain(&[0x19, 0x01]).chain(&domain.separator()).chain(&data.hash()).finalize()
}

/// Why wallet typed data could not be hashed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadTypedData {
    #[error("not JSON: {0}")]
    Json(#[from] BadJson),
    #[error("{0}")]
    Shape(String),
}

/// What a wallet hashes for `eth_signTypedData_v4` JSON, read from the JSON
/// alone: the type strings from its `types` lists, the values from `domain`
/// and `message`. A cross-check of [`TypedStruct::wallet_json`] that shares
/// nothing with it but keccak. Flat structs and the types used here only.
pub fn digest_of_wallet_json(text: &str) -> Result<[u8; 32], BadTypedData> {
    let shape = |what: &str| BadTypedData::Shape(what.to_owned());
    let json = Json::parse(text)?;
    let types = json.get("types").ok_or_else(|| shape("no `types`"))?;
    let primary = json.get("primaryType").and_then(Json::as_str).ok_or_else(|| shape("no `primaryType`"))?;
    let list = |name: &str| {
        types.get(name).and_then(Json::as_array).ok_or_else(|| BadTypedData::Shape(format!("no type `{name}`")))
    };
    let domain = json.get("domain").ok_or_else(|| shape("no `domain`"))?;
    let message = json.get("message").ok_or_else(|| shape("no `message`"))?;
    let separator = struct_hash("EIP712Domain", list("EIP712Domain")?, domain)?;
    let data = struct_hash(primary, list(primary)?, message)?;
    Ok(Keccak256::new().chain(&[0x19, 0x01]).chain(&separator).chain(&data).finalize())
}

fn struct_hash(name: &str, fields: &[Json], values: &Json) -> Result<[u8; 32], BadTypedData> {
    let mut signature = format!("{name}(");
    let mut words = Vec::with_capacity(fields.len());
    for (index, field) in fields.iter().enumerate() {
        let (Some(field_name), Some(kind)) =
            (field.get("name").and_then(Json::as_str), field.get("type").and_then(Json::as_str))
        else {
            return Err(BadTypedData::Shape(format!("a field of `{name}` without a name and a type")));
        };
        if index > 0 {
            signature.push(',');
        }
        signature.push_str(kind);
        signature.push(' ');
        signature.push_str(field_name);
        let value = values
            .get(field_name)
            .ok_or_else(|| BadTypedData::Shape(format!("`{name}` has no value for `{field_name}`")))?;
        let word = encode_json(kind, value)
            .ok_or_else(|| BadTypedData::Shape(format!("`{field_name}` is not a `{kind}`: {}", value.compact())))?;
        words.push(word);
    }
    signature.push(')');
    let mut sponge = Keccak256::new().chain(&keccak256(signature.as_bytes()));
    for word in &words {
        sponge = sponge.chain(word);
    }
    Ok(sponge.finalize())
}

fn encode_json(kind: &str, value: &Json) -> Option<[u8; 32]> {
    match (kind, value) {
        ("string", Json::String(text)) => Some(keccak256(text.as_bytes())),
        ("address", Json::String(text)) => text.parse().ok().map(address_word),
        ("bool", Json::Bool(flag)) => Some(word(u64::from(*flag))),
        ("bytes32", Json::String(text)) => hex::decode_array(text).ok(),
        ("uint256[]", Json::Array(items)) => {
            let mut sponge = Keccak256::new();
            for item in items {
                sponge = sponge.chain(&encode_json("uint256", item)?);
            }
            Some(sponge.finalize())
        }
        // A wallet takes a number either way: a JSON number or its decimal string.
        (_, Json::Number(digits) | Json::String(digits)) => {
            let bits: usize = kind.strip_prefix("uint")?.parse().ok()?;
            let value = decimal_word(digits)?;
            let spare = 256usize.checked_sub(bits)? / 8;
            value[..spare].iter().all(|byte| *byte == 0).then_some(value)
        }
        _ => None,
    }
}

fn word(value: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    bytes
}

fn address_word(address: Address) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(address.bytes());
    bytes
}

#[cfg(test)]
#[path = "tests/eip712.rs"]
mod tests;
