//! Wallet typed data hashed back from its JSON: the same digest for every field kind, and a
//! different one for any changed value or order.

use super::*;
use crate::client::Network;
use crate::utils::sign::eip712::{Domain, Field, TypedStruct};

fn chain() -> Field<'static> {
    Field::String("Mainnet")
}

fn every_kind() -> TypedStruct<'static> {
    TypedStruct::new("HyperliquidTransaction:Everything")
        .field("hyperliquidChain", chain())
        .field("label", Field::String("a \"quoted\" name\n\u{e9}"))
        .field("who", Field::Address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap()))
        .field("wei", Field::Uint64(u64::MAX))
        .field("id", Field::Bytes32([0x5a; 32]))
        .field("toPerp", Field::Bool(false))
        .field("nonce", Field::Uint64(1_790_412_105_001))
}

/// The same digest for every field kind under either domain, and a different one for any changed
/// value or order.
#[test]
fn wallet_json_hashes_back_to_the_digest_and_to_another_for_any_change() {
    for domain in [Domain::user_signed(Network::Mainnet), Domain::EXCHANGE] {
        let data = every_kind();
        let json = data.wallet_json(&domain);
        assert_eq!(digest_of_wallet_json(&json), Ok(data.digest(&domain)), "{json}");
    }
    let data = every_kind();
    let json = data.wallet_json(&Domain::user_signed(Network::Mainnet));
    let right = data.digest(&Domain::user_signed(Network::Mainnet));
    for (from, to) in [
        (r#""toPerp":false"#, r#""toPerp":true"#),
        (r#""chainId":42161"#, r#""chainId":1"#),
        ("1790412105001", "1790412105002"),
        (r#""hyperliquidChain":"Mainnet""#, r#""hyperliquidChain":"mainnet""#),
        (
            r#"{"name":"toPerp","type":"bool"},{"name":"nonce","type":"uint64"}"#,
            r#"{"name":"nonce","type":"uint64"},{"name":"toPerp","type":"bool"}"#,
        ),
    ] {
        assert!(json.contains(from), "{from}");
        let changed = json.replacen(from, to, 1);
        assert_ne!(digest_of_wallet_json(&changed), Ok(right), "{from} -> {to}");
    }
    for broken in [
        r#"{"types":{}}"#.to_owned(),
        "not json".to_owned(),
        json.replacen(r#""toPerp":false"#, r#""toPerp":"false""#, 1),
        json.replacen(r#""primaryType""#, r#""primary""#, 1),
        json.replacen(r#""message""#, r#""messages""#, 1),
        json.replacen(r#""toPerp":false,"#, "", 1),
        json.replacen(r#"{"name":"toPerp","type":"bool"}"#, r#"{"name":"toPerp"}"#, 1),
    ] {
        assert!(digest_of_wallet_json(&broken).is_err(), "{broken}");
    }
}
