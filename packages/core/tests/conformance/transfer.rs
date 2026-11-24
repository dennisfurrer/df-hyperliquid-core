//! Every transfer vector on file, through the crate's own constructors and plans, on the network
//! each case was made for: the posted JSON, the digest where the file has one, and the signature.
//! The files are in `vectors/`, each naming the implementation that made it.
//!
//! - `nktkas-0.33.2.json`: `@nktkas/hyperliquid` 0.33.2, every action on mainnet and on testnet.
//! - `send_asset.json`: `agentSendAsset`, `@nktkas/hyperliquid` 0.33.1, signed by an agent
//!   (Hardhat's first account).
//! - `to_evm.json`: four recorded Core to HyperEVM cases.

use df_hyperliquid_core::account::{Keeping, LocalKey, Signers, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::transfer::{
    AgentSendAsset, Amount, Balance, Change, Ctx, HYPE_SYSTEM, PERP_DEX, SPOT_DEX, SendAsset, SpotOut, SpotSend,
    SpotToken, UsdClassTransfer, UsdSend, Withdraw3,
};
use df_hyperliquid_core::utils::sign::{L1Action, UserAction, digest_of_wallet_json};
use df_hyperliquid_core::utils::{Address, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
const SEND_ASSET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/send_asset.json"));
const TO_EVM: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/to_evm.json"));

/// Hardhat's first account, the agent `send_asset.json` signs with: a public
/// test key that holds nothing.
const HARDHAT_0: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

const OWN: &str = "0x14791697260E4c9A71f18484C9f997B308e59325";
const FRIEND: &str = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414";
/// HYPE as that SDK run named it, on both networks.
const HYPE: &str = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";

/// The transfers the crate builds, by the name the file gives them.
const NAMES: [&str; 5] = ["usdClassTransfer", "usdSend", "spotSend", "sendAsset", "withdraw3"];

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

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn amount(typed: &str) -> Amount {
    Amount::parse(typed).unwrap()
}

fn signature_of(case: &Json) -> (String, String, u8) {
    let expected = case.get("signature").unwrap();
    let v = expected.get("v").and_then(Json::as_u64).unwrap();
    (text(expected, "r").to_owned(), text(expected, "s").to_owned(), u8::try_from(v).unwrap())
}

/// Built from typed values: nothing is read out of the case it is checked against.
fn built(name: &str, nonce: u64) -> Box<dyn UserAction> {
    match name {
        "usdClassTransfer" => Box::new(UsdClassTransfer::new(amount("12.50"), true, nonce)),
        "usdSend" => Box::new(UsdSend::new(address(FRIEND), amount("7"), nonce)),
        "spotSend" => Box::new(SpotSend::new(address(FRIEND), HYPE, amount("0.5"), nonce)),
        "sendAsset" => Box::new(SendAsset::new(HYPE_SYSTEM, SPOT_DEX, SPOT_DEX, HYPE, amount("1.25"), nonce)),
        "withdraw3" => Box::new(Withdraw3::new(address(OWN), amount("25"), nonce)),
        other => panic!("no such transfer {other}"),
    }
}

/// The posted JSON, the digest, the wallet's JSON, the signature and the body.
fn check_user(name: &str, case: &Json, action: &dyn UserAction, key: &LocalKey, nonce: u64) {
    let network = network(case);
    let json = case.get("action").unwrap().compact();
    assert_eq!(action.wire(network), json, "{name} on {network}: the posted action");
    assert_eq!(action.nonce(), nonce, "{name}");
    let digest = action.digest(network);
    if let Some(Json::String(hash)) = case.get("signingHash") {
        assert_eq!(hex::encode_prefixed(&digest), *hash, "{name} on {network}: the digest");
    }
    assert_eq!(digest_of_wallet_json(&action.wallet_json(network)), Ok(digest), "{name} on {network}: wallet json");
    let signature = key.sign_digest(&digest).unwrap();
    let (r, s, v) = signature_of(case);
    assert_eq!((signature.r_hex(), signature.s_hex(), signature.v()), (r.clone(), s.clone(), v), "{name} on {network}");
    assert_eq!(recover(&digest, &signature), Ok(key.address()), "{name} on {network}");
    assert_eq!(
        action.body(&signature, network),
        format!(r#"{{"action":{json},"nonce":{nonce},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#),
        "{name} on {network}: the body"
    );
}

#[test]
fn every_nktkas_transfer_signs_on_mainnet_and_testnet() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    assert_eq!(key.address(), address(OWN));
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("userSigned")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| NAMES.contains(&text(case, "name")))
        .collect();
    assert_eq!(cases.len(), 10, "five transfers, each on both networks");
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 5);
    for case in cases {
        let name = text(case, "name");
        check_user(name, case, built(name, nonce).as_ref(), &key, nonce);
    }
}

/// The same actions come out of the plans.
#[test]
fn the_plans_make_the_nktkas_actions() {
    let file = parse(NKTKAS);
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    for network in Network::ALL {
        let ctx = Ctx { network, account: address(OWN), keeping: Keeping::Apart, nonce };
        let hype: SpotToken = serde_json::from_str(
            r#"{"name":"HYPE","index":150,"tokenId":"0x0d01dc56dcaaca66ad901c959b4011ec","weiDecimals":8,"evmContract":null}"#,
        )
        .unwrap();
        let class = ctx.class_transfer(amount("12.50"), true).unwrap();
        assert_eq!(class.action.wire(network), built("usdClassTransfer", nonce).wire(network));
        let send = ctx.usd_send(address(FRIEND), amount("7")).unwrap();
        assert_eq!(send.action.wire(network), built("usdSend", nonce).wire(network));
        let token = ctx.send_token(address(FRIEND), &hype, amount("0.5")).unwrap();
        assert_eq!(token.action.wire(network), built("spotSend", nonce).wire(network));
        let out = ctx.withdraw(address(OWN), amount("25")).unwrap();
        assert_eq!(out.action.wire(network), built("withdraw3", nonce).wire(network));
        for key in [class.key, send.key, token.key, out.key] {
            assert_eq!(key, Signers::Main, "{network}: every user-signed transfer needs the account's own key");
        }
    }
}

/// `agentSendAsset` is an L1 action: the agent signs the phantom agent over
/// its hash, on mainnet here (source `a`), and the action carries its nonce.
#[test]
fn an_agent_moves_between_the_owner_s_balances_as_recorded() {
    let file = parse(SEND_ASSET);
    let agent = LocalKey::from_hex(HARDHAT_0).unwrap();
    assert_eq!(agent.address(), address(text(&file, "agent")));
    let owner = address(text(&file, "owner"));
    let usdc = SpotToken::from(Network::Mainnet.usdc());
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let name = text(case, "name");
        let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
        let wire = case.get("action").unwrap();
        let (from, to) = (text(wire, "sourceDex"), text(wire, "destinationDex"));
        let typed = match name {
            "spot_to_perps" => amount("96.73"),
            "perps_to_spot" => amount("12.5"),
            "spot_to_xyz" => amount("25"),
            "xyz_to_perps" => amount("1.5"),
            other => panic!("no such case {other}"),
        };
        let action = AgentSendAsset::new(owner, from, to, Network::Mainnet.usdc().wire, typed.clone(), nonce);
        assert_eq!(action.wire(), wire.compact(), "{name}: the posted action");
        assert_eq!(action.nonce(), nonce, "{name}");
        let digest = action.digest(nonce, None, Network::Mainnet);
        let signature = agent.sign_digest(&digest).unwrap();
        let (r, s, v) = signature_of(case);
        assert_eq!((signature.r_hex(), signature.s_hex(), signature.v()), (r, s, v), "{name}: the signature");
        assert_eq!(recover(&digest, &signature), Ok(agent.address()), "{name}");

        // The recorded body is the SDK's; this one adds `"vaultAddress":null`, which the venue reads as none.
        let recorded = parse(text(case, "body"));
        let ours = parse(&action.body(nonce, &signature, None));
        for key in ["action", "nonce", "signature"] {
            assert_eq!(ours.get(key), recorded.get(key), "{name}: the body's {key}");
        }
        assert_eq!(ours.get("vaultAddress"), Some(&Json::Null), "{name}");

        // The plan makes the same action, for an agent.
        let ctx = Ctx { network: Network::Mainnet, account: owner, keeping: Keeping::Apart, nonce };
        let plan = ctx.agent_between(from, to, &usdc, typed.clone()).unwrap();
        assert_eq!(plan.action, action, "{name}");
        assert_eq!((plan.key, plan.nonce), (Signers::Either, nonce), "{name}");
        let source = match from {
            SPOT_DEX => Balance::Spot(0),
            dex => Balance::Perp(dex.to_owned()),
        };
        assert_eq!((plan.effect.balance, plan.effect.change), (source, Change::Down(typed.value())), "{name}");
    }
    assert_eq!(PERP_DEX, "", "the main dex, as the cases spell it");
}

/// The token each `to_evm.json` case crosses, with what the caller's spot meta
/// says of it: HYPE is native on HyperEVM, UBTC is linked to an ERC-20 with two
/// fewer decimals (mainnet `spotMeta`, 2026-10-08).
fn crossing(case: &Json) -> SpotToken {
    let token = case.get("token").unwrap();
    let name = text(token, "name");
    let (wei_decimals, evm_contract) = match name {
        "HYPE" => (8, serde_json::Value::Null),
        "UBTC" => (
            10,
            serde_json::json!({ "address": "0x9fdbda0a5e284c32744d2f17ee5c74b284993463", "evm_extra_wei_decimals": -2 }),
        ),
        other => panic!("no such token {other}"),
    };
    serde_json::from_value(serde_json::json!({
        "name": name,
        "index": token.get("index").and_then(Json::as_u64).unwrap(),
        "tokenId": text(token, "tokenId"),
        "weiDecimals": wei_decimals,
        "evmContract": evm_contract,
    }))
    .unwrap()
}

/// A pooled account crosses by `sendAsset` from spot to spot, a separate one by `spotSend`, each
/// to the token's system address.
#[test]
fn a_pooled_account_crosses_to_hyperevm_by_send_asset_and_a_separate_one_by_spot_send() {
    let file = parse(TO_EVM);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 4);
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 2);
    for case in cases {
        let name = text(case, "name");
        let network = network(case);
        let keeping = match case.get("pooled") {
            Some(Json::Bool(true)) => Keeping::Unified,
            _ => Keeping::Apart,
        };
        let wire = case.get("action").unwrap();
        let token = crossing(case);
        let typed = match token.name.as_str() {
            "HYPE" => amount("1.25"),
            _ => amount("0.001"),
        };
        let ctx = Ctx { network, account: address(OWN), keeping, nonce };
        let plan = ctx.to_evm(&token, typed.clone()).unwrap();
        assert_eq!(plan.action.wire(network), wire.compact(), "{name}: the posted action");
        assert_eq!(plan.key, Signers::Main, "{name}");
        assert_eq!(plan.effect.balance, Balance::Spot(token.index), "{name}");
        assert_eq!(plan.effect.change, Change::DownAtLeast(typed.value()), "{name}");
        let direct = SpotOut::new(token.system_address(network), &token.wire(), typed, keeping, nonce);
        assert_eq!(plan.action, direct, "{name}");

        let digest = plan.action.digest(network);
        assert_eq!(digest_of_wallet_json(&plan.action.wallet_json(network)), Ok(digest), "{name}");
        let signature = key.sign_digest(&digest).unwrap();
        assert_eq!((signature.r_hex(), signature.s_hex(), signature.v()), signature_of(case), "{name}: the signature");
    }
}
