//! A flat typed struct: its type string, its hash, the digest a key signs under a domain, and the
//! typed data a browser wallet's `eth_signTypedData_v4` is handed.

use crate::utils::json::Json;
use crate::utils::sign::eip712::domain::Domain;
use crate::utils::sign::eip712::field::Field;
use crate::utils::sign::keccak::{Keccak256, keccak256};

/// An EIP-712 struct with no nested struct types: a primary type name and its fields in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStruct<'a> {
    primary_type: &'a str,
    fields: Vec<(&'a str, Field<'a>)>,
}

impl<'a> TypedStruct<'a> {
    /// A struct named `primary_type`, such as `HyperliquidTransaction:UsdSend`, with no fields yet.
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

    /// The struct's type name.
    #[must_use]
    pub const fn primary_type(&self) -> &'a str {
        self.primary_type
    }

    /// Each field's name and value, in the order they were added.
    #[must_use]
    pub fn fields(&self) -> &[(&'a str, Field<'a>)] {
        &self.fields
    }

    /// The type string, such as `Agent(string source,bytes32 connectionId)`.
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

    /// keccak256 of [`TypedStruct::type_signature`].
    #[must_use]
    pub fn type_hash(&self) -> [u8; 32] {
        keccak256(self.type_signature().as_bytes())
    }

    /// The struct hash: keccak256 of the type hash followed by each field's word.
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

#[cfg(test)]
#[path = "tests/typed_struct.rs"]
mod tests;
