//! Wallet typed data hashed from its JSON alone, sharing nothing with how it was written but keccak.

pub use super::typed::BadTypedData;

/// What a wallet hashes for `eth_signTypedData_v4` JSON, read from the JSON
/// alone: the type strings from its `types` lists, the values from `domain`
/// and `message`, structs inside structs and arrays included
/// ([`super::typed::read`]). A cross-check of
/// [`TypedStruct::wallet_json`](super::TypedStruct::wallet_json) that shares
/// nothing with it but keccak.
pub fn digest_of_wallet_json(text: &str) -> Result<[u8; 32], BadTypedData> {
    super::typed::digest(text)
}

#[cfg(test)]
#[path = "tests/wallet.rs"]
mod tests;
