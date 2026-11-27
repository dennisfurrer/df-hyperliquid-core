//! hyperliquid-sdk-rs benchmarks (github.com/lhermoso/hyperliquid-rust-sdk), the same
//! subcommands and output as `bench-df`: `sign-check`, `sign`, `parse --corpus <path>`,
//! `serve --px <price> --sz <size>` and `live --secs <n>`.
//!
//! hyperliquid-sdk-rs has no public call that signs an order without sending it, and it takes the
//! nonce from the clock. The offline benchmarks therefore run the steps of
//! `RawExchangeProvider::place_order` one by one, with its public types and with a copy of its
//! private `hash_action`, so the fixed nonce can be used. The signature check confirms the result
//! is the signature its client would send.

use std::error::Error;
use std::time::{Duration, Instant};

use alloy::primitives::{Address, B256, keccak256};
use alloy::signers::local::PrivateKeySigner;
use bench_common::{
    CountingAlloc, FIXED_ASSET, FIXED_KEY, FIXED_NONCE, FIXED_PX, FIXED_SZ, LiveTally, args, flag, load_corpus,
    parse_report, sign_samples, sign_warmup, summary_ns, time_each, warm,
};
use hyperliquid_sdk_rs::signers::{HyperliquidSignature, HyperliquidSigner, SignerError};
use hyperliquid_sdk_rs::types::ws::Message;
use hyperliquid_sdk_rs::types::{Agent, BulkOrder, ExchangeResponseStatus, HyperliquidAction, OrderRequest};
use hyperliquid_sdk_rs::{ExchangeProvider, InfoProvider, Network, WsProvider};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const SDK: &str = "hyperliquid-sdk-rs";

fn main() -> Result<()> {
    bench_common::runtime()?.block_on(run())
}

async fn run() -> Result<()> {
    let args = args();
    let report = match args.first().map(String::as_str) {
        Some("sign-check") => sign_check()?,
        Some("sign") => sign()?,
        Some("parse") => parse(&flag(&args, "--corpus").ok_or("--corpus <path>")?)?,
        Some("serve") => {
            let px = flag(&args, "--px").ok_or("--px <price>")?;
            let sz = flag(&args, "--sz").ok_or("--sz <size>")?;
            return serve(px, sz).await;
        }
        Some("live") => live(flag(&args, "--secs").ok_or("--secs <n>")?.parse()?).await?,
        Some("verify-copy") => verify_copy().await?,
        _ => return Err("usage: bench-lhermoso sign-check | sign | parse | serve | live | verify-copy".into()),
    };
    println!("{report}");
    Ok(())
}

/// `RawExchangeProvider::hash_action` for the `order` action. The SDK's version wraps the action
/// in an enum with one variant for each action type; only the `order` variant is used here, and
/// it serializes the same.
fn hash_action<T: Serialize>(action: &T, timestamp: u64, vault_address: Option<Address>) -> Option<B256> {
    #[derive(Serialize)]
    #[serde(tag = "type")]
    #[serde(rename_all = "camelCase")]
    enum ActionWrapper<'a, T> {
        Order(&'a T),
    }
    let wrapped = ActionWrapper::Order(action);
    let mut bytes = rmp_serde::to_vec_named(&wrapped).ok()?;
    bytes.extend(timestamp.to_be_bytes());
    if let Some(vault) = vault_address {
        bytes.push(1);
        bytes.extend(vault.as_slice());
    } else {
        bytes.push(0);
    }
    Some(keccak256(bytes))
}

/// The order `place_order` builds from `OrderRequest::limit`, as the README shows it.
fn bulk_order(px: &str, sz: &str) -> BulkOrder {
    // `place_order` takes the request by reference and clones it.
    let order = OrderRequest::limit(FIXED_ASSET, true, px, sz, "Alo");
    BulkOrder { orders: vec![order.clone()], grouping: "na".to_string(), builder: None }
}

/// The signing steps of `send_l1_action` on mainnet: hash the action, wrap the hash in the
/// `Agent` struct and sign its EIP-712 hash. The SDK's signer is async; its future is ready at
/// once and is polled here on the calling thread.
fn sign_order(signer: &PrivateKeySigner, action: &BulkOrder, nonce: u64) -> Option<HyperliquidSignature> {
    let connection_id = hash_action(action, nonce, None)?;
    let agent = Agent { source: "a".to_string(), connection_id };
    let domain = agent.domain();
    let signing_hash = agent.eip712_signing_hash(&domain);
    futures::executor::block_on(HyperliquidSigner::sign_hash(signer, signing_hash)).ok()
}

/// The action JSON `send_l1_action` posts: the action with its `type` added.
fn action_value(action: &BulkOrder) -> Option<Value> {
    let mut value = serde_json::to_value(action).ok()?;
    if let Value::Object(ref mut map) = value {
        map.insert("type".to_string(), json!("order"));
    }
    Some(value)
}

/// A signer for `verify-copy`: it keeps the hash the SDK asks it to sign and the signature the
/// fixed key makes over it, then refuses, so `place_order` stops before posting anything.
struct Recording {
    inner: PrivateKeySigner,
    seen: std::sync::Arc<std::sync::Mutex<Option<(B256, HyperliquidSignature)>>>,
}

// `HyperliquidSigner` is an `async_trait`; this is the method that macro would generate.
impl HyperliquidSigner for Recording {
    fn sign_hash<'life0, 'async_trait>(
        &'life0 self,
        hash: B256,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = std::result::Result<HyperliquidSignature, SignerError>>
                + Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let signature = HyperliquidSigner::sign_hash(&self.inner, hash).await?;
            if let Ok(mut seen) = self.seen.lock() {
                *seen = Some((hash, signature));
            }
            Err(SignerError::Unavailable)
        })
    }

    fn address(&self) -> Address {
        self.inner.address()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

/// Checks the copied `hash_action` against the SDK's own. The SDK's public `place_order` runs
/// with a signer that records the EIP-712 hash it is asked to sign and then refuses, so nothing is
/// posted. The SDK takes the nonce from the clock, so the copy is run for every millisecond the
/// call spanned; one of them must give the same hash, and the copy's signature at that nonce must
/// be the same bytes as the signature over the SDK's hash.
async fn verify_copy() -> Result<Value> {
    // The provider builds its HTTP client at once, which needs a TLS provider, though nothing is sent.
    install_crypto_provider();
    let key: PrivateKeySigner = FIXED_KEY.parse()?;
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let exchange = ExchangeProvider::mainnet(Recording { inner: key.clone(), seen: std::sync::Arc::clone(&seen) });
    let order = OrderRequest::limit(FIXED_ASSET, true, FIXED_PX, FIXED_SZ, "Alo");
    let before = now_ms();
    // The recording signer refuses, so this is an error by design.
    let _ = exchange.place_order(&order).await;
    let after = now_ms();
    let (sdk_hash, sdk_signature) = seen.lock().map_err(|_| "lock")?.take().ok_or("the SDK asked for no signature")?;
    let action = bulk_order(FIXED_PX, FIXED_SZ);
    for nonce in before..=after {
        let connection_id = hash_action(&action, nonce, None).ok_or("hash failed")?;
        let agent = Agent { source: "a".to_string(), connection_id };
        if agent.eip712_signing_hash(&agent.domain()) != sdk_hash {
            continue;
        }
        let copy = sign_order(&key, &action, nonce).ok_or("signing failed")?;
        let equal = (copy.r, copy.s, copy.v) == (sdk_signature.r, sdk_signature.s, sdk_signature.v);
        return Ok(json!({
            "sdk": SDK,
            "verify_copy": {
                "equal": equal,
                "nonce": nonce,
                "sdk_signing_hash": sdk_hash.to_string(),
                "sdk_signature": { "r": sdk_signature.r.to_string(), "s": sdk_signature.s.to_string(), "v": sdk_signature.v },
                "copy_signature": { "r": copy.r.to_string(), "s": copy.s.to_string(), "v": copy.v },
            },
        }));
    }
    Ok(
        json!({ "sdk": SDK, "verify_copy": { "equal": false, "reason": "no nonce in the call's span gave the SDK's hash" } }),
    )
}

/// The body `RawExchangeProvider::post` sends.
fn body(action: Value, signature: &HyperliquidSignature, nonce: u64) -> Option<Vec<u8>> {
    let payload = json!({
        "action": action,
        "signature": {
            "r": format!("0x{:064x}", signature.r),
            "s": format!("0x{:064x}", signature.s),
            "v": signature.v,
        },
        "nonce": nonce,
        "vaultAddress": Option::<Address>::None,
    });
    serde_json::to_vec(&payload).ok()
}

fn hex65(signature: &HyperliquidSignature) -> String {
    format!("0x{:064x}{:064x}{:02x}", signature.r, signature.s, signature.v)
}

fn sign_check() -> Result<Value> {
    let signer: PrivateKeySigner = FIXED_KEY.parse()?;
    let action = bulk_order(FIXED_PX, FIXED_SZ);
    let signature = sign_order(&signer, &action, FIXED_NONCE).ok_or("signing failed")?;
    Ok(json!({
        "sdk": SDK,
        "address": format!("{:#x}", signer.address()),
        "signature": hex65(&signature),
        "action": action_value(&action).ok_or("no action JSON")?,
    }))
}

fn sign() -> Result<Value> {
    let signer: PrivateKeySigner = FIXED_KEY.parse()?;
    let action = bulk_order(FIXED_PX, FIXED_SZ);

    // Hash and sign an order action that is already built.
    let hash_and_sign = || sign_order(&signer, &action, FIXED_NONCE);
    warm(sign_warmup(), hash_and_sign);
    let signed = time_each(sign_samples(), hash_and_sign);

    // From price and size strings to the JSON body `/exchange` takes.
    let build_sign_body = || -> Option<Vec<u8>> {
        let action = bulk_order(FIXED_PX, FIXED_SZ);
        let signature = sign_order(&signer, &action, FIXED_NONCE)?;
        body(action_value(&action)?, &signature, FIXED_NONCE)
    };
    warm(sign_warmup(), build_sign_body);
    let bodies = time_each(sign_samples(), build_sign_body);

    Ok(json!({ "sdk": SDK, "sign": summary_ns(&signed), "build_sign_body": summary_ns(&bodies) }))
}

fn parse(path: &str) -> Result<Value> {
    let corpus = load_corpus(path)?;
    // What the websocket provider's message router does with each text frame: it owns the
    // frame's bytes (copied out of the websocket buffer) and decodes them in place with
    // simd-json into its `Message` type.
    let report = parse_report(&corpus, |text| {
        let mut bytes = text.as_bytes().to_vec();
        simd_json::from_slice::<Message>(&mut bytes).is_ok()
    });
    Ok(json!({ "sdk": SDK, "parse": report }))
}

/// What the SDK's examples do first: pick rustls's aws-lc-rs provider for TLS.
fn install_crypto_provider() {
    let _ = rustls::crypto::CryptoProvider::install_default(rustls::crypto::aws_lc_rs::default_provider());
}

async fn serve(px: String, sz: String) -> Result<()> {
    install_crypto_provider();
    // A fresh random key that has never held anything: the venue has no account for it.
    let signer = PrivateKeySigner::random();
    let address = signer.address();
    let exchange = ExchangeProvider::mainnet(signer);
    let info = InfoProvider::mainnet();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let answer = match line.trim() {
            "info" => {
                let start = Instant::now();
                let read = info.l2_book("BTC").await;
                let ns = start.elapsed().as_nanos() as u64;
                match read {
                    Ok(book) => {
                        let (bids, asks) =
                            (book.levels.first().map_or(0, Vec::len), book.levels.get(1).map_or(0, Vec::len));
                        json!({"ok": true, "ns": ns, "answer": format!("{bids} bids {asks} asks")})
                    }
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "exchange" => {
                let start = Instant::now();
                let order = OrderRequest::limit(FIXED_ASSET, true, px.as_str(), sz.as_str(), "Alo");
                let placed = exchange.place_order(&order).await;
                let ns = start.elapsed().as_nanos() as u64;
                let refused = |text: &str| text.contains("does not exist") || text.contains("User or API Wallet");
                match placed {
                    Ok(ExchangeResponseStatus::Ok(response)) => {
                        json!({"ok": true, "ns": ns, "answer": format!("accepted: {response:?}")})
                    }
                    // A venue refusal is the expected answer here.
                    Ok(ExchangeResponseStatus::Err(text)) => json!({"ok": refused(&text), "ns": ns, "answer": text}),
                    Err(error) => {
                        let text = error.to_string();
                        json!({"ok": refused(&text), "ns": ns, "answer": text})
                    }
                }
            }
            "address" => json!({"ok": true, "ns": 0, "answer": format!("{address:#x}")}),
            "quit" | "" => break,
            other => json!({"ok": false, "ns": 0, "answer": format!("unknown command {other}")}),
        };
        stdout.write_all(format!("{answer}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

async fn live(secs: u64) -> Result<Value> {
    install_crypto_provider();
    let mut tally = LiveTally::default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ws = WsProvider::connect(Network::Mainnet).await?;
    tally.connects += 1;
    // The l2Book subscription has no `fast` option. The router sends every message to every
    // subscription's channel, so one channel carries all three; the other two are dropped.
    let (_, mut messages) = ws.subscribe_l2_book("BTC").await?;
    let _ = ws.subscribe_bbo("BTC").await?;
    let _ = ws.subscribe_trades("BTC").await?;
    ws.start_reading().await?;
    while let Ok(Some(message)) = tokio::time::timeout_at(deadline, messages.recv()).await {
        match message {
            Message::L2Book(book) => {
                tally.book_frames += 1;
                tally.aged(book.data.time);
            }
            Message::Bbo(bbo) => {
                tally.bbo_frames += 1;
                tally.aged(bbo.data.time);
            }
            Message::Trades(trades) => {
                tally.trade_frames += 1;
                tally.trades += trades.data.len() as u64;
                if let Some(newest) = trades.data.iter().map(|trade| trade.time).max() {
                    tally.aged(newest);
                }
            }
            _ => tally.other_frames += 1,
        }
    }
    let mut report = tally.report(secs);
    // The router drops a frame it cannot read without a trace, so they cannot be counted.
    report["unreadable"] = Value::Null;
    Ok(json!({ "sdk": SDK, "live": report }))
}
