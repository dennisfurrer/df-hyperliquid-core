//! Wallet typed data hashed from its JSON alone, sharing nothing with how it was written but keccak.

use crate::utils::hex;
use crate::utils::json::{BadJson, Json};
use crate::utils::sign::eip712::word::{address_word, decimal_word, word};
use crate::utils::sign::keccak::{Keccak256, keccak256};

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
/// and `message`. A cross-check of
/// [`TypedStruct::wallet_json`](super::TypedStruct::wallet_json) that shares
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

#[cfg(test)]
#[path = "tests/wallet.rs"]
mod tests;
