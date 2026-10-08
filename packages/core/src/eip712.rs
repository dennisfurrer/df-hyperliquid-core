//! EIP-712 for the flat structs Hyperliquid signs: hashing, and the typed
//! data a browser wallet's `eth_signTypedData_v4` is handed.

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
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 32] {
        match self {
            Field::String(text) => keccak256(text.as_bytes()),
            Field::Address(address) => address_word(*address),
            Field::Uint64(number) => word(*number),
            Field::Bytes32(bytes) => *bytes,
            Field::Bool(value) => word(u64::from(*value)),
        }
    }

    /// The value as a wallet's `message` carries it: numbers as JSON numbers,
    /// as viem passes them, bools as JSON bools, the rest as strings.
    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Field::String(text) => Json::String((*text).to_owned()),
            Field::Address(address) => Json::String(address.to_hex()),
            Field::Uint64(number) => Json::Number(number.to_string()),
            Field::Bytes32(bytes) => Json::String(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Json::Bool(*value),
        }
    }

    /// The value as the posted action carries it: addresses lowercase.
    #[must_use]
    pub fn to_wire(&self) -> Value {
        match self {
            Field::String(text) => Value::from((*text).to_owned()),
            Field::Address(address) => Value::from(address.to_hex()),
            Field::Uint64(number) => Value::Uint(*number),
            Field::Bytes32(bytes) => Value::from(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Value::Bool(*value),
        }
    }
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
        ("uint64" | "uint256", Json::Number(digits)) => {
            let number: u128 = digits.parse().ok()?;
            let mut bytes = [0u8; 32];
            bytes[16..].copy_from_slice(&number.to_be_bytes());
            Some(bytes)
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
