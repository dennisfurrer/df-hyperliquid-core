//! `@nktkas/hyperliquid` 0.33.2's vectors (`vectors/nktkas-0.33.2.json`) through the signing
//! code alone: each L1 action exactly as written (key order kept), its hash and its signature on
//! the network it was made for, and a user-signed action on both networks. The other L1 files are
//! rebuilt through the typed builders in `trade/`.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::utils::sign::user_signed::NonceKey;
use df_hyperliquid_core::utils::sign::{L1Action, UserSigned};
use df_hyperliquid_core::utils::{Address, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn network(case: &Json) -> Network {
    match case.get("isTestnet") {
        Some(Json::Bool(true)) => Network::Testnet,
        Some(Json::Bool(false)) => Network::Mainnet,
        other => panic!("isTestnet is {other:?}"),
    }
}

/// Some files write `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

/// One case: msgpack where the file has it, the hash, then the signature.
fn check_l1(name: &str, case: &Json, key: &LocalKey, nonce: u64) {
    let action = case.get("action").unwrap().to_action().unwrap();
    if let Some(Json::String(packed)) = case.get("msgpack") {
        assert_eq!(hex::encode_prefixed(&action.msgpack()), *packed, "{name}: msgpack");
    }
    let vault = match case.get("vaultAddress") {
        Some(Json::String(vault)) => Some(vault.parse::<Address>().unwrap()),
        _ => None,
    };
    let network = network(case);
    assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, vault)), text(case, "actionHash"), "{name}: hash");
    let signature = key.sign_digest(&action.digest(nonce, vault, network)).unwrap();
    let expected = case.get("signature").unwrap();
    assert_eq!(signature.r_hex(), word(text(expected, "r")), "{name} on {network}: r");
    assert_eq!(signature.s_hex(), word(text(expected, "s")), "{name} on {network}: s");
    assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{name}: v");
}

#[test]
fn every_nktkas_l1_action_signs_on_mainnet_and_testnet() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases = file.get("l1").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 16);
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 8);
    for case in cases {
        check_l1(text(case, "name"), case, &key, nonce);
    }
}

/// `usdClassTransfer` on both networks, built by hand: the chain names, the digest and the
/// signature all follow the network. The rest of the user-signed vectors run through the modules
/// that build those actions.
#[test]
fn a_user_signed_action_follows_its_network() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("userSigned")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| text(case, "name") == "usdClassTransfer")
        .collect();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let network = network(case);
        let action =
            UserSigned::new("usdClassTransfer", "HyperliquidTransaction:UsdClassTransfer", NonceKey::Nonce, nonce)
                .field("amount", df_hyperliquid_core::utils::sign::Field::String("12.5"))
                .field("toPerp", df_hyperliquid_core::utils::sign::Field::Bool(true));
        assert_eq!(action.wire(network), case.get("action").unwrap().compact(), "{network}: wire");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{network}: digest");
        assert_eq!(
            df_hyperliquid_core::utils::sign::eip712::digest_of_wallet_json(&action.wallet_json(network)),
            Ok(digest),
            "{network}"
        );
        let signature = key.sign_digest(&digest).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), text(expected, "r"), "{network}: r");
        assert_eq!(signature.s_hex(), text(expected, "s"), "{network}: s");
    }
}

/// The approval the hip4 terminal's builder popup signs: 880 tenths of a basis point as `0.88%`,
/// as `@nktkas/hyperliquid` 0.33.2's `approveBuilderFee` makes it with `{maxFeeRate: "0.88%",
/// builder}`, signed by the Python SDK's test key on each network.
#[test]
fn a_builder_approval_at_point_eight_eight_percent_is_the_terminal_s() {
    use df_hyperliquid_core::account::ApproveBuilderFee;
    use df_hyperliquid_core::trade::builder::fee::BuilderFee;
    use df_hyperliquid_core::utils::sign::UserAction;

    let key = LocalKey::from_hex("0x0123456789012345678901234567890123456789012345678901234567890123").unwrap();
    let builder: Address = "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843".parse().unwrap();
    let approval = ApproveBuilderFee::new(builder, BuilderFee::from_tenths(880), 1_790_412_105_001);
    assert_eq!(approval.max_fee_rate(), "0.88%");
    for (network, wire, digest, r, s, v) in [
        (
            Network::Mainnet,
            r#"{"type":"approveBuilderFee","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","maxFeeRate":"0.88%","builder":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","nonce":1790412105001}"#,
            "0xad59f180d28d38a190641cbd95ac679eb9e3d20068781a38a1e4aa3b0ed07c01",
            "0x53482b31f09b90b7a956030466b1b119eb4fb4afe78403a8d3c4982e5d8f49b9",
            "0x55bf1ab58c69d99d0551881194bc9a1b308c1ebf8c17a41066e32b20390f6027",
            28,
        ),
        (
            Network::Testnet,
            r#"{"type":"approveBuilderFee","signatureChainId":"0x66eee","hyperliquidChain":"Testnet","maxFeeRate":"0.88%","builder":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","nonce":1790412105001}"#,
            "0xa66ca231fbd79d79e3eba61c1c7c31ee92d07e88473075219809670556b4fc10",
            "0x016069ea6a0dc2d8124a4ef55221f807a103afb78a0e735698744e6dbeb527c5",
            "0x143a02f8e285d77e0abcd9ea7a76d0eeb1e561a317b07ba1d98f58261e2b8fd8",
            28,
        ),
    ] {
        let signed = approval.user_signed();
        assert_eq!(signed.wire(network), wire, "{network:?}: the posted action");
        assert_eq!(hex::encode_prefixed(&signed.digest(network)), digest, "{network:?}: the digest");
        let signature = key.sign_digest(&signed.digest(network)).unwrap();
        assert_eq!((signature.r_hex(), signature.s_hex(), signature.v()), (r.to_owned(), s.to_owned(), v));
    }
}
