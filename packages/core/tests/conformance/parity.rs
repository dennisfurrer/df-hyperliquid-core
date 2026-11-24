//! The actions added since `@nktkas/hyperliquid` 0.33.2, against `vectors/nktkas-0.33.3-parity.json`:
//! each built through this crate's constructors on the network its case was made for, then its
//! posted JSON, msgpack, hash or digest, and signature compared with the SDK's. The venue's Python
//! SDK 0.24.0 reproduces every signature in the file. `cDeposit2` and `trailingStop` have no SDK
//! function: their cases are the docs' shape signed by the SDK's `signL1Action`.

use df_hyperliquid_core::account::{ClaimRewards, ConvertToMultiSigUser, LocalKey, ReserveRequestWeight, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::earn::staking::CDeposit2;
use df_hyperliquid_core::earn::vaults::Hip3LiquidatorTransfer;
use df_hyperliquid_core::trade::outcomes::{MergeQuestion, NegateOutcome};
use df_hyperliquid_core::trade::{Asset, Retracement, Side, TrailingStop};
use df_hyperliquid_core::transfer::send::Amount;
use df_hyperliquid_core::transfer::{Destination, SendToEvmWithData};
use df_hyperliquid_core::utils::sign::{L1Action, UserAction, digest_of_wallet_json};
use df_hyperliquid_core::utils::{Address, Dec, Json, hex};

const PARITY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.3-parity.json"));

const DEST: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";

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

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// The file's key, nonce and the cases under `list`.
fn cases(list: &str) -> (LocalKey, u64, Vec<Json>) {
    let file = Json::parse(PARITY).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    (key, nonce, file.get(list).and_then(Json::as_array).unwrap().to_vec())
}

fn signature_of(case: &Json) -> (String, String, u64) {
    let expected = case.get("signature").unwrap();
    (text(expected, "r").to_owned(), text(expected, "s").to_owned(), expected.get("v").and_then(Json::as_u64).unwrap())
}

/// Each L1 case from typed inputs, the ones the vectors were made with.
fn l1(name: &str) -> Box<dyn L1Action> {
    match name {
        "claimRewards" => Box::new(ClaimRewards),
        "reserveRequestWeight" => Box::new(ReserveRequestWeight::new(1_000).unwrap()),
        "reserveRequestWeight.destination" => {
            Box::new(ReserveRequestWeight::new(1_000).unwrap().for_user(address(DEST)))
        }
        "hip3LiquidatorTransfer.deposit" => Box::new(Hip3LiquidatorTransfer::deposit("xyz", dec("1000")).unwrap()),
        "hip3LiquidatorTransfer.withdraw" => Box::new(Hip3LiquidatorTransfer::withdraw("xyz", dec("5000")).unwrap()),
        "mergeQuestion" => Box::new(MergeQuestion::new(51, dec("12.50")).unwrap()),
        "mergeQuestion.all" => Box::new(MergeQuestion::all(51)),
        "negateOutcome" => Box::new(NegateOutcome::new(51, 1473, dec("3")).unwrap()),
        "cDeposit2" => Box::new(CDeposit2::new(dec("100")).unwrap()),
        "trailingStop.pct" => Box::new(
            TrailingStop::new(Asset::from_id(0), Side::Sell, dec("0.01"), Retracement::Percent(dec("1.5")))
                .unwrap()
                .reduce_only(),
        ),
        "trailingStop.px" => Box::new(
            TrailingStop::new(Asset::from_id(110_001), Side::Buy, dec("2"), Retracement::Price(dec("2.5")))
                .and_then(|stop| stop.activated_at(dec("410")))
                .unwrap(),
        ),
        other => panic!("no such case {other}"),
    }
}

fn user_signed(name: &str, nonce: u64) -> Box<dyn UserAction> {
    let signers =
        [address("0x70997970c51812dc3a010c7d01b50e0d17dc79c8"), address("0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc")];
    match name {
        "sendToEvmWithData" => Box::new(SendToEvmWithData::new(
            "USDC:0x6d1e7cde53ba9467b783cb7c530ce054",
            Amount::parse("10.5").unwrap(),
            "",
            Destination::evm(address(DEST), 999),
            200_000,
            vec![0x12, 0x34, 0xab, 0xcd],
            nonce,
        )),
        "sendToEvmWithData.empty" => Box::new(SendToEvmWithData::new(
            "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec",
            Amount::parse("1").unwrap(),
            "spot",
            Destination::base58("11111111111111111111111111111111", 1),
            0,
            Vec::new(),
            nonce,
        )),
        "convertToMultiSigUser" => Box::new(ConvertToMultiSigUser::new(&signers, 2, nonce).unwrap()),
        "convertToMultiSigUser.single" => Box::new(ConvertToMultiSigUser::single(nonce)),
        other => panic!("no such case {other}"),
    }
}

#[test]
fn every_new_l1_action_hashes_and_signs_as_the_sdk_does_on_mainnet_and_testnet() {
    let (key, nonce, cases) = cases("l1");
    assert_eq!(cases.len(), 22, "eleven actions, each on both networks");
    for case in &cases {
        let (name, network) = (text(case, "name"), network(case));
        let action = l1(name);
        assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}: the posted action");
        assert_eq!(hex::encode_prefixed(&action.msgpack()), text(case, "msgpack"), "{name}: msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}: hash");
        let digest = action.digest(nonce, None, network);
        let signature = key.sign_digest(&digest).unwrap();
        assert_eq!(
            (signature.r_hex(), signature.s_hex(), u64::from(signature.v())),
            signature_of(case),
            "{name} on {network}: the signature"
        );
        assert_eq!(recover(&digest, &signature), Ok(key.address()));
    }
}

#[test]
fn every_new_user_signed_action_signs_as_the_sdk_does_on_mainnet_and_testnet() {
    let (key, nonce, cases) = cases("userSigned");
    assert_eq!(cases.len(), 8, "four actions, each on both networks");
    for case in &cases {
        let (name, network) = (text(case, "name"), network(case));
        let action = user_signed(name, nonce);
        let json = case.get("action").unwrap().compact();
        assert_eq!(action.wire(network), json, "{name} on {network}: the posted action");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{name} on {network}: the digest");
        assert_eq!(digest_of_wallet_json(&action.wallet_json(network)), Ok(digest), "{name} on {network}");
        let signature = key.sign_digest(&digest).unwrap();
        let (r, s, v) = signature_of(case);
        assert_eq!((signature.r_hex(), signature.s_hex(), u64::from(signature.v())), (r.clone(), s.clone(), v));
        assert_eq!(
            action.body(&signature, network),
            format!(r#"{{"action":{json},"nonce":{nonce},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#),
            "{name} on {network}: the body"
        );
    }
}
