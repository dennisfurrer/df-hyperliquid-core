//! The `/exchange` body, keys in the official Python SDK's order. The venue
//! reads it as JSON, so only the action's own key order reaches a hash.

use crate::account::Signature;
use crate::utils::address::Address;
use crate::utils::json::Json;
use crate::utils::sign::msgpack::Map;

/// `{"action":..,"nonce":..,"signature":{r,s,v}}`, for a user-signed action.
#[must_use]
pub fn user_signed(action: &Map, nonce: u64, signature: &Signature) -> String {
    Json::Object(vec![
        ("action".to_owned(), action.to_json()),
        ("nonce".to_owned(), Json::Number(nonce.to_string())),
        ("signature".to_owned(), signature.json()),
    ])
    .compact()
}

/// [`user_signed`] plus `"vaultAddress"`, an explicit `null` unless the action
/// acts for a vault or subaccount, for an L1 action.
#[must_use]
pub fn l1(action: &Map, nonce: u64, signature: &Signature, vault: Option<Address>) -> String {
    Json::Object(vec![
        ("action".to_owned(), action.to_json()),
        ("nonce".to_owned(), Json::Number(nonce.to_string())),
        ("signature".to_owned(), signature.json()),
        ("vaultAddress".to_owned(), vault.map_or(Json::Null, |vault| Json::String(vault.to_hex()))),
    ])
    .compact()
}
