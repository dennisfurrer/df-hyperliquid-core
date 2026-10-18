//! Functions and events by their signature: a [`Function`]'s selector, calldata
//! and answer, and the topic an event is logged under.

use crate::client::evm::abi::kind::is_name;
use crate::client::evm::abi::{AbiError, Param, Token, decode, encode, params, tuple_type};
use crate::client::evm::u256::B256;
use crate::utils::hex;
use crate::utils::sign::keccak256;

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

#[cfg(test)]
#[path = "tests/function.rs"]
mod tests;
