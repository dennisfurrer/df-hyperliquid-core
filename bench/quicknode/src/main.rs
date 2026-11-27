//! quicknode-hyperliquid-sdk benchmarks, the same subcommands and output as `bench-df`:
//! `sign-check`, `sign`, `parse --corpus <path>`, `serve --px <price> --sz <size>` and
//! `live --secs <n>`.
//!
//! The SDK's order methods do not hash the action locally: they send it to QuickNode's worker
//! (send.hyperliquidapi.com), which returns the hash, sign that hash, and send the order through
//! the worker. The offline benchmarks time the SDK's public `signing::sign_action`, which hashes
//! and signs locally, on the action JSON its `place_order` builds for a limit order, without the
//! random client order id it adds (the other rows sign no client order id either).
//!
//! `serve` and `live` use the QuickNode endpoint in `QUICKNODE_ENDPOINT` when it is set. The
//! runner does not start `live` without one, because `Stream` is documented for a QuickNode
//! endpoint.

use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alloy::signers::local::PrivateKeySigner;
use bench_common::{
    CountingAlloc, FIXED_ASSET, FIXED_KEY, FIXED_NONCE, FIXED_PX, FIXED_SZ, LiveTally, args, flag, load_corpus,
    parse_report, sign_samples, sign_warmup, summary_ns, time_each, warm,
};
use hyperliquid_sdk::signing::sign_action;
use hyperliquid_sdk::{Chain, HyperliquidSDK, Order, Signature, Stream};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const SDK: &str = "quicknode-hyperliquid-sdk";

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
            let px: f64 = flag(&args, "--px").ok_or("--px <price>")?.parse()?;
            let sz: f64 = flag(&args, "--sz").ok_or("--sz <size>")?.parse()?;
            return serve(px, sz).await;
        }
        Some("live") => live(flag(&args, "--secs").ok_or("--secs <n>")?.parse()?).await?,
        _ => return Err("usage: bench-quicknode sign-check | sign | parse | serve | live".into()),
    };
    println!("{report}");
    Ok(())
}

/// The limit order action `HyperliquidSDK::place_order` builds: size rounded to the asset's size
/// decimals (5 for BTC, also the SDK's fallback), price rounded to a whole number, both written
/// with `format!("{}")`. The SDK adds a random client order id `c`; it is left out here.
fn action(px: f64, sz: f64) -> Value {
    let sz_decimals = 5;
    let size_rounded = (sz * 10f64.powi(sz_decimals)).round() / 10f64.powi(sz_decimals);
    let resolved_price = px.round();
    json!({
        "type": "order",
        "orders": [{
            "a": FIXED_ASSET,
            "b": true,
            "p": format!("{}", resolved_price),
            "s": format!("{}", size_rounded),
            "r": false,
            "t": {"limit": {"tif": "Alo"}},
        }],
        "grouping": "na",
    })
}

/// `signing::sign_action` on mainnet with no vault and no expiry. It is async; its future is
/// ready at once and is polled here on the calling thread.
fn sign_order(signer: &PrivateKeySigner, action: &Value, nonce: u64) -> Option<Signature> {
    futures::executor::block_on(sign_action(signer, Chain::Mainnet, action, nonce, None, None)).ok()
}

fn fixed() -> Result<(PrivateKeySigner, f64, f64)> {
    let signer: PrivateKeySigner = FIXED_KEY.trim_start_matches("0x").parse()?;
    Ok((signer, FIXED_PX.parse()?, FIXED_SZ.parse()?))
}

fn hex65(signature: &Signature) -> String {
    format!("0x{:064x}{:064x}{:02x}", signature.r, signature.s, signature.v)
}

fn sign_check() -> Result<Value> {
    let (signer, px, sz) = fixed()?;
    let action = action(px, sz);
    let signature = sign_order(&signer, &action, FIXED_NONCE).ok_or("signing failed")?;
    Ok(json!({
        "sdk": SDK,
        "address": format!("{:#x}", signer.address()),
        "signature": hex65(&signature),
        "action": action,
    }))
}

fn sign() -> Result<Value> {
    let (signer, px, sz) = fixed()?;
    let built = action(px, sz);

    // Hash and sign an order action that is already built.
    let hash_and_sign = || sign_order(&signer, &built, FIXED_NONCE);
    warm(sign_warmup(), hash_and_sign);
    let signed = time_each(sign_samples(), hash_and_sign);

    // From price and size to the JSON body `send_action` posts.
    let build_sign_body = || -> Option<Vec<u8>> {
        let action = action(px, sz);
        let signature = sign_order(&signer, &action, FIXED_NONCE)?;
        let body = json!({ "action": action, "nonce": FIXED_NONCE, "signature": signature });
        serde_json::to_vec(&body).ok()
    };
    warm(sign_warmup(), build_sign_body);
    let bodies = time_each(sign_samples(), build_sign_body);

    Ok(json!({ "sdk": SDK, "sign": summary_ns(&signed), "build_sign_body": summary_ns(&bodies) }))
}

fn parse(path: &str) -> Result<Value> {
    let corpus = load_corpus(path)?;
    // What `Stream` does with each text frame: read it into a `serde_json::Value`, with the
    // `preserve_order` feature the SDK turns on. It then hands every callback its own clone,
    // which is not timed here.
    let report = parse_report(&corpus, |text| serde_json::from_str::<Value>(text).is_ok());
    Ok(json!({ "sdk": SDK, "parse": report }))
}

/// The QuickNode endpoint from `QUICKNODE_ENDPOINT`, when one is set.
fn endpoint() -> Option<String> {
    std::env::var("QUICKNODE_ENDPOINT").ok().filter(|endpoint| !endpoint.trim().is_empty())
}

async fn serve(px: f64, sz: f64) -> Result<()> {
    // A fresh random key that has never held anything: the venue has no account for it.
    let signer = PrivateKeySigner::random();
    let key = format!("0x{}", hex::encode(signer.to_bytes()));
    // Without a QuickNode endpoint (`QUICKNODE_ENDPOINT`), info requests go to QuickNode's public
    // worker, which forwards them to the venue. Orders go through the worker either way.
    let mut builder = HyperliquidSDK::new().private_key(key);
    if let Some(endpoint) = endpoint() {
        builder = builder.endpoint(endpoint);
    }
    let sdk = builder.build().await?;
    let info = sdk.info();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let answer = match line.trim() {
            "info" => {
                let start = Instant::now();
                let read = info.l2_book("BTC", None, None).await;
                let ns = start.elapsed().as_nanos() as u64;
                match read {
                    Ok(book) => {
                        let side = |at: usize| book["levels"][at].as_array().map_or(0, Vec::len);
                        json!({"ok": true, "ns": ns, "answer": format!("{} bids {} asks", side(0), side(1))})
                    }
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "exchange" => {
                let start = Instant::now();
                let placed = sdk.order(Order::buy("BTC").size(sz).price(px).alo()).await;
                let ns = start.elapsed().as_nanos() as u64;
                match placed {
                    Ok(order) => json!({"ok": true, "ns": ns, "answer": format!("accepted: {order:?}")}),
                    // A venue refusal is the expected answer here.
                    Err(error) => {
                        let text = error.to_string();
                        let refused = text.contains("does not exist") || text.contains("User or API Wallet");
                        json!({"ok": refused, "ns": ns, "answer": text})
                    }
                }
            }
            "address" => json!({"ok": true, "ns": 0, "answer": format!("{:#x}", signer.address())}),
            "quit" | "" => break,
            other => json!({"ok": false, "ns": 0, "answer": format!("unknown command {other}")}),
        };
        stdout.write_all(format!("{answer}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

fn count(tally: &Mutex<LiveTally>, refused: &Mutex<Option<String>>, frame: &Value) {
    if frame["channel"] == "error"
        && let Ok(mut refused) = refused.lock()
        && refused.is_none()
    {
        *refused = frame["data"].as_str().map(str::to_owned);
    }
    let Ok(mut tally) = tally.lock() else { return };
    let data = &frame["data"];
    match frame["channel"].as_str() {
        Some("l2Book") => {
            tally.book_frames += 1;
            if let Some(time) = data["time"].as_u64() {
                tally.aged(time);
            }
        }
        Some("bbo") => {
            tally.bbo_frames += 1;
            if let Some(time) = data["time"].as_u64() {
                tally.aged(time);
            }
        }
        Some("trades") => {
            tally.trade_frames += 1;
            let trades = data.as_array().map_or(&[][..], Vec::as_slice);
            tally.trades += trades.len() as u64;
            if let Some(newest) = trades.iter().filter_map(|trade| trade["time"].as_u64()).max() {
                tally.aged(newest);
            }
        }
        _ => tally.other_frames += 1,
    }
}

async fn live(secs: u64) -> Result<Value> {
    let tally = Arc::new(Mutex::new(LiveTally::default()));
    // The first error the venue sends back, if any: without a QuickNode endpoint, `Stream` sends
    // its subscriptions to the public websocket in a shape the venue does not accept.
    let refused = Arc::new(Mutex::new(None::<String>));
    let opened = Arc::clone(&tally);
    let mut stream = Stream::new(endpoint()).on_open(move || {
        if let Ok(mut tally) = opened.lock() {
            tally.connects += 1;
        }
    });
    // `Stream` hands every frame to every callback, so only the first one counts.
    let (counted, noted) = (Arc::clone(&tally), Arc::clone(&refused));
    stream.l2_book("BTC", move |frame| count(&counted, &noted, &frame));
    stream.bbo("BTC", |_| {});
    stream.trades(&["BTC"], |_| {});
    stream.start()?;
    tokio::time::sleep(Duration::from_secs(secs)).await;
    stream.stop();
    let mut report = tally.lock().map_err(|_| "tally lock")?.report(secs);
    // `Stream` drops a frame it cannot read without a trace, so they cannot be counted.
    report["unreadable"] = Value::Null;
    report["venue_error"] = Value::from(refused.lock().map_err(|_| "error lock")?.clone());
    Ok(json!({ "sdk": SDK, "live": report }))
}
