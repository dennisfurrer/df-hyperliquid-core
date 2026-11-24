//! df-hyperliquid-core benchmarks. Each subcommand prints one JSON object on stdout:
//!
//! - `sign-check`: the signature of the fixed order, for the cross-SDK check.
//! - `sign`: hash and sign the fixed order, and build, sign and serialize it.
//! - `parse --corpus <path>`: decode the recorded websocket frames.
//! - `serve --px <price> --sz <size>`: answer `info` and `exchange` lines on stdin, one JSON
//!   line each, for the paced network rounds `run.py` drives.
//! - `live --secs <n>`: BTC `l2Book` and `trades` from mainnet for `n` seconds.

use std::error::Error;
use std::time::{Duration, Instant};

use bench_common::{
    FIXED_ASSET, FIXED_KEY, FIXED_NONCE, FIXED_PX, FIXED_SZ, LiveTally, SIGN_SAMPLES, SIGN_WARMUP, args, flag,
    load_corpus, parse_report, summary_ns, time_each, warm,
};
use df_hyperliquid_core::account::{LocalKey, Signature};
use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::client::native::{Feed, Native, clock};
use df_hyperliquid_core::live::{Event, Frame, Session, Subscription, decode};
use df_hyperliquid_core::markets::{Precision, book};
use df_hyperliquid_core::trade::{Asset, L1Action, Order, PlaceOrder, Side, Tif};
use df_hyperliquid_core::{Client, Dec, Network, SendError};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args();
    let report = match args.first().map(String::as_str) {
        Some("sign-check") => sign_check()?,
        Some("sign") => sign()?,
        Some("parse") => parse(&flag(&args, "--corpus").ok_or("--corpus <path>")?)?,
        Some("serve") => {
            let px: Dec = flag(&args, "--px").ok_or("--px <price>")?.parse()?;
            let sz: Dec = flag(&args, "--sz").ok_or("--sz <size>")?.parse()?;
            return serve(px, sz).await;
        }
        Some("live") => live(flag(&args, "--secs").ok_or("--secs <n>")?.parse()?).await?,
        _ => return Err("usage: bench-df sign-check | sign | parse | serve | live".into()),
    };
    println!("{report}");
    Ok(())
}

fn fixed_order() -> Result<PlaceOrder> {
    let asset = Asset::perp(FIXED_ASSET).ok_or("not a perp")?;
    let px: Dec = FIXED_PX.parse()?;
    let sz: Dec = FIXED_SZ.parse()?;
    Ok(PlaceOrder::one(Order::limit(asset, Side::Buy, px, sz, Tif::Alo)))
}

fn hex65(signature: &Signature) -> String {
    format!("{}{}{:02x}", signature.r_hex(), &signature.s_hex()[2..], signature.v())
}

fn sign_check() -> Result<Value> {
    let key = LocalKey::from_hex(FIXED_KEY)?;
    let place = fixed_order()?;
    let signature = key.sign_digest(&place.digest(FIXED_NONCE, None, Network::Mainnet))?;
    Ok(json!({
        "sdk": "df-hyperliquid-core",
        "address": key.address().to_hex(),
        "signature": hex65(&signature),
        "action": serde_json::from_str::<Value>(&place.wire())?,
    }))
}

fn sign() -> Result<Value> {
    let key = LocalKey::from_hex(FIXED_KEY)?;
    let place = fixed_order()?;
    let asset = Asset::perp(FIXED_ASSET).ok_or("not a perp")?;
    let px: Dec = FIXED_PX.parse()?;
    let sz: Dec = FIXED_SZ.parse()?;

    // Hash and sign an order action that is already built: msgpack, keccak, EIP-712, ECDSA.
    let hash_and_sign = || key.sign_digest(&place.digest(FIXED_NONCE, None, Network::Mainnet));
    warm(SIGN_WARMUP, hash_and_sign);
    let signed = time_each(SIGN_SAMPLES, hash_and_sign);

    // From typed price and size to the JSON body `/exchange` takes.
    let build_sign_body = || -> Option<String> {
        let place = PlaceOrder::one(Order::limit(asset, Side::Buy, px, sz, Tif::Alo));
        let signature = key.sign_digest(&place.digest(FIXED_NONCE, None, Network::Mainnet)).ok()?;
        Some(place.body(FIXED_NONCE, &signature, None))
    };
    warm(SIGN_WARMUP, build_sign_body);
    let bodies = time_each(SIGN_SAMPLES, build_sign_body);

    Ok(json!({
        "sdk": "df-hyperliquid-core",
        "sign": summary_ns(&signed),
        "build_sign_body": summary_ns(&bodies),
    }))
}

fn parse(path: &str) -> Result<Value> {
    let corpus = load_corpus(path)?;
    let report = parse_report(&corpus, |text| decode(text).is_ok());
    Ok(json!({ "sdk": "df-hyperliquid-core", "parse": report }))
}

/// A fresh random key that has never held anything: the venue has no account for it.
fn throwaway_key() -> Result<LocalKey> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|error| error.to_string())?;
    Ok(LocalKey::from_bytes(&secret)?)
}

async fn serve(px: Dec, sz: Dec) -> Result<()> {
    let key = throwaway_key()?;
    let client = Client::new(Native::new(), Network::Mainnet, clock, key);
    let asset = Asset::perp(FIXED_ASSET).ok_or("not a perp")?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let answer = match line.trim() {
            "info" => {
                let start = Instant::now();
                let read = book::l2_book(&client, "BTC", Precision::FULL).await;
                let ns = start.elapsed().as_nanos() as u64;
                match read {
                    Ok(book) => {
                        json!({"ok": true, "ns": ns, "answer": format!("{} bids {} asks", book.bids.len(), book.asks.len())})
                    }
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "exchange" => {
                let start = Instant::now();
                let place = PlaceOrder::one(Order::limit(asset, Side::Buy, px, sz, Tif::Alo));
                let sent = client.send(&place).await;
                let ns = start.elapsed().as_nanos() as u64;
                match sent {
                    // The venue answered and the answer was read: a refusal is the expected answer.
                    Ok(accepted) => json!({"ok": true, "ns": ns, "answer": format!("accepted: {accepted:?}")}),
                    Err(SendError::Api(ApiError::Refused(reason))) => json!({"ok": true, "ns": ns, "answer": reason}),
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "address" => json!({"ok": true, "ns": 0, "answer": client.signer().address().to_hex()}),
            "quit" | "" => break,
            other => json!({"ok": false, "ns": 0, "answer": format!("unknown command {other}")}),
        };
        stdout.write_all(format!("{answer}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

async fn live(secs: u64) -> Result<Value> {
    let mut tally = LiveTally::default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut feed = Feed::start(Session::new(Network::Mainnet));
    let btc = || "BTC".to_owned();
    feed.subscribe(Subscription::L2Book { coin: btc(), n_sig_figs: None, mantissa: None, fast: false }).await?;
    feed.subscribe(Subscription::Trades { coin: btc() }).await?;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, feed.next()).await {
        match event {
            Event::Opened => tally.connects += 1,
            Event::Frame(frame) => match *frame {
                Frame::Book(book) => {
                    tally.book_frames += 1;
                    tally.aged(book.time);
                }
                Frame::Trades(trades) => {
                    tally.trade_frames += 1;
                    tally.trades += trades.len() as u64;
                    if let Some(newest) = trades.iter().map(|trade| trade.time).max() {
                        tally.aged(newest);
                    }
                }
                _ => tally.other_frames += 1,
            },
            Event::Unreadable(_) => tally.unreadable += 1,
            Event::Closed { .. } => {}
        }
    }
    feed.close();
    Ok(json!({ "sdk": "df-hyperliquid-core", "live": tally.report(secs) }))
}
