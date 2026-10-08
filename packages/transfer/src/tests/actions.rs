//! Every vector below was made by `@nktkas/hyperliquid` 0.33.2 itself, as
//! hip4-tui recorded it: its own action functions, run with the key below,
//! nonce 1790412105001 and a transport that recorded the body instead of
//! posting it. The digests are viem's `hashTypedData` over the SDK's own
//! EIP-712 types. Mainnet rows only.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{
    Dec, LocalKey, Signature, Transport, TransportError, client::exchange, digest_of_wallet_json, hex, recover, verify,
};
use serde_json::Value;

use super::*;
use crate::{
    amount::Amount,
    check::{HYPE_SYSTEM, pre_transfer_check},
    ledger::{Delta, ledger},
};

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const OWN: &str = "0x14791697260E4c9A71f18484C9f997B308e59325";
const FRIEND: &str = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414";
const HYPE: &str = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";
const NONCE: u64 = 1_790_412_105_001;

fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// Typed amounts through [`Amount::parse`], as the app takes them.
fn amount(typed: &str) -> Amount {
    Amount::parse(typed).unwrap()
}

/// `(action, posted json, digest or "" where the SDK run kept none, r, s, v)`.
type Vector = (Box<dyn UserAction>, &'static str, &'static str, &'static str, &'static str, u8);

fn vectors() -> Vec<Vector> {
    vec![
        (
            Box::new(UsdClassTransfer::new(amount("12.50"), true, NONCE)),
            r#"{"type":"usdClassTransfer","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","amount":"12.5","toPerp":true,"nonce":1790412105001}"#,
            "0xba62924d6d47cc0d43658f10c6ac3967b714968a44cabfcffafa320e1cc62b9e",
            "0xace29947ac808dffad68f296903b5385a996a47146c80b8c26035b798148f7c3",
            "0x65f57493988af9c6dbcc96a96fdebf0d2451f51c3aac8640410273ef22e5889a",
            27,
        ),
        (
            Box::new(UsdSend::new(address(FRIEND), amount("7"), NONCE)),
            r#"{"type":"usdSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","amount":"7","time":1790412105001}"#,
            "0x1e017ef7c0e62cec3881e381829d61d20397dd7c3e8883ead2ae2f5c3fc0860d",
            "0x80fd99805788b66a4d1b210cb716898e5a6984675eeeb3ed140b2c86f6b72f74",
            "0x759d1987b09a7f5f77161a3538be62e8e6e79e6fd8789f5981909584c9ba8673",
            27,
        ),
        (
            Box::new(SpotSend::new(address(FRIEND), HYPE, amount("0.5"), NONCE)),
            r#"{"type":"spotSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","token":"HYPE:0x0d01dc56dcaaca66ad901c959b4011ec","amount":"0.5","time":1790412105001}"#,
            "0xa4ba0ab820ba7a125552808b0a94dd8873651f23254b49ff616d0f5d1a514a36",
            "0xa94f0aaadf851c107ab6933b609808973134ac509ce54f41928b72d92bee2f72",
            "0x6a1a8f9b652a9706d4a43ef1ae3a9cce490a219161561a19b202c40eec2ed1eb",
            28,
        ),
        (
            Box::new(SendAsset::new(HYPE_SYSTEM, SPOT_DEX, SPOT_DEX, HYPE, amount("1.25"), NONCE)),
            r#"{"type":"sendAsset","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x2222222222222222222222222222222222222222","sourceDex":"spot","destinationDex":"spot","token":"HYPE:0x0d01dc56dcaaca66ad901c959b4011ec","amount":"1.25","fromSubAccount":"","nonce":1790412105001}"#,
            "0x1c3638895ab8e3af7a5b80e5d4559c7dd785234db8bbb9b244c4f9001c3ad01c",
            "0x3bb51363bd34911a924b348552ca77ab2cec97636fc2041a6ac26e871aec950d",
            "0x73237f95de58344ba61f4de55300936b2394e02573c93b8c8b6e3b79c065ab45",
            28,
        ),
        (
            Box::new(Withdraw3::new(address(OWN), amount("25"), NONCE)),
            r#"{"type":"withdraw3","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x14791697260e4c9a71f18484c9f997b308e59325","amount":"25","time":1790412105001}"#,
            "0xeecd9c81fc6b341775776975171d22445742a202e4ceaa2b9b4a71adc89809e1",
            "0xe526e1053384258551b936f6ac802e188a4d1549e08f672df386e92ffdbe5e5c",
            "0x4883250dcd34fbbf77985e0a6bf4090eee753d1b59c33dab5fc5ff697cce9e05",
            27,
        ),
        // HYPE to HyperEVM both ways from the same SDK run: `spotSend` for a
        // standard account, `sendAsset` spot to spot for a pooled one.
        (
            Box::new(SpotSend::new(HYPE_SYSTEM, "HYPE:0x00000000000000000000000000000096", amount("1.25"), NONCE)),
            r#"{"type":"spotSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x2222222222222222222222222222222222222222","token":"HYPE:0x00000000000000000000000000000096","amount":"1.25","time":1790412105001}"#,
            "",
            "0x827f978f3534efae70fed4a2516a69a7b96ea8f70b66d0f1e3d5a9b0aab79b81",
            "0x190e955ce2485993005aa0cb898d337b585a38cf51ee79d7c491662d49773e6e",
            27,
        ),
        (
            Box::new(SendAsset::new(
                HYPE_SYSTEM,
                SPOT_DEX,
                SPOT_DEX,
                "HYPE:0x00000000000000000000000000000096",
                amount("1.25"),
                NONCE,
            )),
            r#"{"type":"sendAsset","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x2222222222222222222222222222222222222222","sourceDex":"spot","destinationDex":"spot","token":"HYPE:0x00000000000000000000000000000096","amount":"1.25","fromSubAccount":"","nonce":1790412105001}"#,
            "",
            "0x1aee352fd1d8f494df97b39a4d5aff7f4635d4adf8ac71964d0dab9916ec5b9b",
            "0x5597d89e3407222339339b5cccca87be58742bc49005b6e2e145943b009dfe74",
            27,
        ),
    ]
}

#[test]
fn every_transfer_is_the_sdk_s_byte_for_byte() {
    let key = key();
    assert_eq!(key.address(), address(OWN));
    for (action, json, digest, r, s, v) in vectors() {
        assert_eq!(action.wire(Network::Mainnet), json, "the posted action");
        assert_eq!(action.nonce(), NONCE);
        if !digest.is_empty() {
            assert_eq!(hex::encode_prefixed(&action.digest(Network::Mainnet)), digest, "{json}: the digest");
        }
        let signature = key.sign_digest(&action.digest(Network::Mainnet)).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(recover(&action.digest(Network::Mainnet), &signature), Ok(key.address()), "{json}");
        assert_eq!(
            action.body(&signature, Network::Mainnet),
            format!(r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#),
            "{json}: the body"
        );
    }
}

/// The JSON a wallet is handed hashes, read back on its own, to the digest
/// the SDK signed; and it names the same fields, in the same order, with the
/// same values as the posted action.
#[test]
fn every_transfer_s_wallet_json_is_the_digest_and_the_posted_action() {
    for (action, json, _, _, _, _) in vectors() {
        let wallet = action.wallet_json(Network::Mainnet);
        assert_eq!(digest_of_wallet_json(&wallet), Ok(action.digest(Network::Mainnet)), "{wallet}");

        let typed: Value = serde_json::from_str(&wallet).unwrap();
        let posted: Value = serde_json::from_str(json).unwrap();
        let primary = typed["primaryType"].as_str().unwrap();
        assert_eq!(primary, action.typed(Network::Mainnet).primary_type());
        assert_eq!(typed["domain"]["chainId"], 42_161);
        assert_eq!(typed["domain"]["name"], "HyperliquidSignTransaction");

        let names: Vec<&str> =
            typed["types"][primary].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
        let wire = action.action(Network::Mainnet);
        let posted_names: Vec<&str> = wire.keys().filter(|key| !matches!(*key, "type" | "signatureChainId")).collect();
        assert_eq!(names, posted_names, "{json}: the typed fields are the posted keys");
        for name in names {
            assert_eq!(typed["message"][name], posted[name], "{json}: {name}");
        }
    }
}

/// The browser's path, with a local key standing in for the wallet: it is
/// handed the JSON, signs what the JSON hashes to, and answers 65 bytes of hex.
#[test]
fn a_wallet_signature_is_read_checked_and_posted() {
    let key = key();
    let send = SendAsset::usdc(Network::Mainnet, address(FRIEND), SPOT_DEX, SPOT_DEX, amount("25"), NONCE);
    assert_eq!(
        send.wire(Network::Mainnet),
        r#"{"type":"sendAsset","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","sourceDex":"spot","destinationDex":"spot","token":"USDC:0x6d1e7cde53ba9467b783cb7c530ce054","amount":"25","fromSubAccount":"","nonce":1790412105001}"#
    );
    let wallet_digest = digest_of_wallet_json(&send.wallet_json(Network::Mainnet)).unwrap();
    let signed = key.sign_digest(&wallet_digest).unwrap();
    let mut answer = [0u8; 65];
    answer[..64].copy_from_slice(&signed.to_compact());
    answer[64] = signed.v() - 27;
    let signature = Signature::from_hex(&hex::encode_prefixed(&answer)).unwrap();
    assert_eq!(verify(&send.digest(Network::Mainnet), &signature, address(OWN)), Ok(()));
    assert!(
        verify(&send.digest(Network::Mainnet), &signature, address(FRIEND)).is_err(),
        "a wallet signing as another account"
    );
    assert!(send.body(&signature, Network::Mainnet).starts_with(r#"{"action":{"type":"sendAsset","#));
}

#[test]
fn moving_between_one_s_own_balances_is_a_send_asset_to_oneself() {
    let combine = SendAsset::usdc(
        Network::Mainnet,
        address(OWN),
        PERP_DEX,
        SPOT_DEX,
        Amount::cut(Dec::parse("12.3456789").unwrap(), 6).unwrap(),
        NONCE,
    );
    assert!(combine.wire(Network::Mainnet).contains(
        r#""destination":"0x14791697260e4c9a71f18484c9f997b308e59325","sourceDex":"","destinationDex":"spot","#
    ));
    assert!(combine.wire(Network::Mainnet).contains(r#""amount":"12.345678""#));
    let from_sub = SendAsset::usdc(Network::Mainnet, address(OWN), SPOT_DEX, PERP_DEX, amount("1"), NONCE)
        .from_sub_account(address(FRIEND));
    assert!(
        from_sub.wire(Network::Mainnet).contains(r#""fromSubAccount":"0x0d1d9635d0640821d15e323ac8adadfa9c111414""#)
    );
    assert_ne!(
        from_sub.digest(Network::Mainnet),
        SendAsset::usdc(Network::Mainnet, address(OWN), SPOT_DEX, PERP_DEX, amount("1"), NONCE)
            .digest(Network::Mainnet)
    );
}

/// Answers by path and keeps what was sent. Nothing leaves the process.
struct Recorded {
    info: &'static str,
    exchange: &'static str,
    sent: RefCell<Vec<(String, String)>>,
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        let answer = if path == "/exchange" { self.exchange } else { self.info };
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(Ok(answer.to_owned()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn a_send_checks_the_recipient_then_posts_its_body_once() {
    let transport = Recorded {
        info: r#"{"isSanctioned":false,"userExists":false,"fee":"1.0","userHasSentTx":false}"#,
        exchange: r#"{"status":"ok","response":{"type":"default"}}"#,
        sent: RefCell::new(Vec::new()),
    };
    let check = block_on(pre_transfer_check(&transport, address(FRIEND), address(OWN))).unwrap();
    assert_eq!(check.activation_fee(), Some(Dec::ONE));

    let send = SendAsset::usdc(Network::Mainnet, address(FRIEND), SPOT_DEX, SPOT_DEX, amount("25"), NONCE);
    let signature = key().sign_digest(&send.digest(Network::Mainnet)).unwrap();
    let accepted = block_on(exchange(&transport, send.body(&signature, Network::Mainnet))).unwrap();
    assert_eq!(accepted.kind, "default");

    let sent = transport.sent.borrow();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].0, "/info");
    assert!(sent[0].1.contains(
        r#""user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","source":"0x14791697260e4c9a71f18484c9f997b308e59325""#
    ));
    assert_eq!(
        (sent[1].0.as_str(), sent[1].1.as_str()),
        ("/exchange", send.body(&signature, Network::Mainnet).as_str())
    );
}

#[test]
fn the_ledger_reads_through_the_transport() {
    let transport = Recorded {
        info: r#"[{"time":1764137107387,"hash":"0x839a943b8933989f37013c2678ae68f5119ec1cf43a1e7eb9e607285a52b3485","delta":{"type":"deposit","usdc":"2525.99"}}]"#,
        exchange: "",
        sent: RefCell::new(Vec::new()),
    };
    let rows = block_on(ledger(&transport, address(OWN), 1_764_000_000_000, None)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].delta, Delta::Deposit { usdc: Dec::parse("2525.99").unwrap() });
    assert_eq!(transport.sent.borrow()[0].0, "/info");
}

#[test]
fn usdc_is_named_by_the_network_s_own_token_id() {
    for network in hl_core::Network::ALL {
        let send = SendAsset::usdc(network, address(FRIEND), SPOT_DEX, SPOT_DEX, amount("25"), NONCE);
        let token = format!(r#""token":"{}""#, network.usdc().wire);
        assert!(send.wire(network).contains(&token), "{network}: {}", send.wire(network));
    }
}
