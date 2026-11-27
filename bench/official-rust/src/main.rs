//! hyperliquid_rust_sdk benchmarks (Hyperliquid's official Rust SDK), the same subcommands and
//! output as `bench-df`: `sign-check`, `sign`, `parse --corpus <path>`,
//! `serve --px <price> --sz <size>` and `live --secs <n>`.
//!
//! The SDK has no public call that signs an order without sending it, it takes the nonce from the
//! clock, and its wire order type and signing functions are private. The offline benchmarks
//! therefore run the steps of `ExchangeClient::bulk_order` one by one: its public `Actions` and
//! `BulkOrder` types, with copies of its private `Actions::hash`, `float_to_string_for_hashing`,
//! `ClientOrderRequest::convert`, `sign_l1_action`, `sign_typed_data`, `sign_hash`, the `Agent`
//! EIP-712 type and the `Sha256Proxy` digest. The signature check confirms the result is the
//! signature its client would send. The copies are marked `#[inline(never)]`: in the SDK they are
//! non-generic functions of another crate, which the compiler does not inline into the caller.

mod proxy_digest;

use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bench_common::{
    args, flag, load_corpus, parse_report, sign_samples, sign_warmup, summary_ns, time_each, warm, CountingAlloc,
    LiveTally, FIXED_ASSET, FIXED_KEY, FIXED_NONCE, FIXED_PX, FIXED_SZ,
};
use ethers::core::k256::{elliptic_curve::FieldBytes, Secp256k1};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::transaction::eip712::Eip712;
use ethers::types::{Signature, H160, H256, U256};
use hyperliquid_rust_sdk::{
    Actions, AssetMeta, BaseUrl, BulkOrder, ClientLimit, ClientOrder, ClientOrderRequest, ExchangeClient,
    ExchangeResponseStatus, InfoClient, Message, Meta, Order, Subscription,
};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::proxy_digest::Sha256Proxy;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const SDK: &str = "hyperliquid_rust_sdk";

/// The websocket manager logs a frame it cannot read at error level and goes on. This logger only
/// counts those lines, so the live run can report them.
struct CountUnparsed;

static UNPARSED: AtomicU64 = AtomicU64::new(0);

impl log::Log for CountUnparsed {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Error
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.args().to_string().starts_with("Error processing data received by WsManager reader") {
            UNPARSED.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

static LOGGER: CountUnparsed = CountUnparsed;

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
        Some("verify-copy") => verify_copy().await?,
        _ => return Err("usage: bench-official-rust sign-check | sign | parse | serve | live | verify-copy".into()),
    };
    println!("{report}");
    Ok(())
}

mod l1 {
    use ethers::contract::{Eip712, EthAbiType};
    use ethers::types::H256;

    /// The SDK's private `signature::agent::l1::Agent`.
    #[derive(Debug, Eip712, Clone, EthAbiType)]
    #[eip712(
        name = "Exchange",
        version = "1",
        chain_id = 1337,
        verifying_contract = "0x0000000000000000000000000000000000000000"
    )]
    pub(crate) struct Agent {
        pub(crate) source: String,
        pub(crate) connection_id: H256,
    }
}

/// The SDK's private `Actions::hash`.
#[inline(never)]
fn hash(action: &Actions, timestamp: u64, vault_address: Option<H160>) -> Option<H256> {
    let mut bytes = rmp_serde::to_vec_named(action).ok()?;
    bytes.extend(timestamp.to_be_bytes());
    if let Some(vault_address) = vault_address {
        bytes.push(1);
        bytes.extend(vault_address.to_fixed_bytes());
    } else {
        bytes.push(0);
    }
    Some(H256(ethers::utils::keccak256(bytes)))
}

/// The SDK's private `sign_l1_action`, `sign_typed_data` and `sign_hash`.
#[inline(never)]
fn sign_l1_action(wallet: &LocalWallet, connection_id: H256, is_mainnet: bool) -> Option<Signature> {
    let source = if is_mainnet { "a" } else { "b" }.to_string();
    let encoded = l1::Agent { source, connection_id }.encode_eip712().ok()?;
    let (sig, rec_id) = wallet.signer().sign_digest_recoverable(Sha256Proxy::from(H256::from(encoded))).ok()?;
    let v = u8::from(rec_id) as u64 + 27;
    let r_bytes: FieldBytes<Secp256k1> = sig.r().into();
    let s_bytes: FieldBytes<Secp256k1> = sig.s().into();
    let r = U256::from_big_endian(r_bytes.as_slice());
    let s = U256::from_big_endian(s_bytes.as_slice());
    Some(Signature { r, s, v })
}

/// The SDK's private `float_to_string_for_hashing`.
#[inline(never)]
fn float_to_string_for_hashing(x: f64) -> String {
    let mut x = format!("{:.*}", 8, x);
    while x.ends_with('0') {
        x.pop();
    }
    if x.ends_with('.') {
        x.pop();
    }
    if x == "-0" {
        "0".to_string()
    } else {
        x
    }
}

/// The SDK's private `ExchangePayload`, the body `ExchangeClient::post` sends.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExchangePayload {
    action: Value,
    signature: Signature,
    nonce: u64,
    vault_address: Option<H160>,
}

/// The SDK does not export its wire order type, so an order is made by cloning this template,
/// read once into the SDK's own `Actions` type, and setting its fields as
/// `ClientOrderRequest::convert` does.
fn template() -> Result<BulkOrder> {
    let order = json!({
        "type": "order",
        "orders": [{"a": 0, "b": true, "p": "0", "s": "0", "r": false, "t": {"limit": {"tif": "Alo"}}}],
        "grouping": "na",
    });
    match serde_json::from_value::<Actions>(order)? {
        Actions::Order(bulk) => Ok(bulk),
        _ => Err("the template is not an order".into()),
    }
}

/// `ClientOrderRequest::convert` and the `Actions::Order` that `bulk_order` wraps it in.
#[inline(never)]
fn convert(template: &BulkOrder, coin_to_asset: &HashMap<String, u32>, order: ClientOrderRequest) -> Option<Actions> {
    let mut bulk = template.clone();
    let wire = bulk.orders.first_mut()?;
    let ClientOrder::Limit(limit) = order.order_type else { return None };
    match &mut wire.order_type {
        Order::Limit(wire_limit) => wire_limit.tif = limit.tif,
        Order::Trigger(_) => return None,
    }
    wire.asset = *coin_to_asset.get(&order.asset)?;
    wire.is_buy = order.is_buy;
    wire.reduce_only = order.reduce_only;
    wire.limit_px = float_to_string_for_hashing(order.limit_px);
    wire.sz = float_to_string_for_hashing(order.sz);
    wire.cloid = None;
    Some(Actions::Order(bulk))
}

/// The order a user hands `ExchangeClient::order`, as the SDK's `order_and_cancel` example writes it.
fn client_order(px: f64, sz: f64) -> ClientOrderRequest {
    ClientOrderRequest {
        asset: "BTC".to_string(),
        is_buy: true,
        reduce_only: false,
        limit_px: px,
        sz,
        cloid: None,
        order_type: ClientOrder::Limit(ClientLimit { tif: "Alo".to_string() }),
    }
}

struct Fixed {
    wallet: LocalWallet,
    px: f64,
    sz: f64,
    template: BulkOrder,
    coin_to_asset: HashMap<String, u32>,
}

fn fixed() -> Result<Fixed> {
    Ok(Fixed {
        wallet: FIXED_KEY.trim_start_matches("0x").parse()?,
        px: FIXED_PX.parse()?,
        sz: FIXED_SZ.parse()?,
        template: template()?,
        coin_to_asset: HashMap::from([("BTC".to_string(), FIXED_ASSET)]),
    })
}

fn sign_check() -> Result<Value> {
    let fixed = fixed()?;
    let action = convert(&fixed.template, &fixed.coin_to_asset, client_order(fixed.px, fixed.sz)).ok_or("no action")?;
    let connection_id = hash(&action, FIXED_NONCE, None).ok_or("hash failed")?;
    let signature = sign_l1_action(&fixed.wallet, connection_id, true).ok_or("signing failed")?;
    Ok(json!({
        "sdk": SDK,
        "address": format!("{:#x}", fixed.wallet.address()),
        "signature": format!("0x{signature}"),
        "action": serde_json::to_value(&action)?,
    }))
}

fn sign() -> Result<Value> {
    let fixed = fixed()?;
    let action = convert(&fixed.template, &fixed.coin_to_asset, client_order(fixed.px, fixed.sz)).ok_or("no action")?;

    // Hash and sign an order action that is already built.
    let hash_and_sign = || sign_l1_action(&fixed.wallet, hash(&action, FIXED_NONCE, None)?, true);
    warm(sign_warmup(), hash_and_sign);
    let signed = time_each(sign_samples(), hash_and_sign);

    // From the user's order to the JSON body `/exchange` takes, in `bulk_order`'s order of steps.
    let build_sign_body = || -> Option<String> {
        let action = convert(&fixed.template, &fixed.coin_to_asset, client_order(fixed.px, fixed.sz))?;
        let connection_id = hash(&action, FIXED_NONCE, None)?;
        let action = serde_json::to_value(&action).ok()?;
        let signature = sign_l1_action(&fixed.wallet, connection_id, true)?;
        let payload = ExchangePayload { action, signature, nonce: FIXED_NONCE, vault_address: None };
        serde_json::to_string(&payload).ok()
    };
    warm(sign_warmup(), build_sign_body);
    let bodies = time_each(sign_samples(), build_sign_body);

    Ok(json!({ "sdk": SDK, "sign": summary_ns(&signed), "build_sign_body": summary_ns(&bodies) }))
}

/// The SDK's private `WsManager::get_identifier` for the two channels in the corpus: the key it
/// looks the subscribers up by.
fn identifier(message: &Message) -> Option<String> {
    match message {
        Message::L2Book(book) => serde_json::to_string(&Subscription::L2Book { coin: book.data.coin.clone() }).ok(),
        Message::Trades(trades) => match trades.data.first() {
            Some(trade) => serde_json::to_string(&Subscription::Trades { coin: trade.coin.clone() }).ok(),
            None => Some(String::new()),
        },
        _ => Some(String::new()),
    }
}

fn parse(path: &str) -> Result<Value> {
    let corpus = load_corpus(path)?;
    // What `WsManager::parse_and_send_data` does with each text frame before it sends the message
    // on: read it into the SDK's `Message` type and make the subscription key for it.
    let report = parse_report(&corpus, |text| {
        text.starts_with('{')
            && serde_json::from_str::<Message>(text).ok().and_then(|message| identifier(&message)).is_some()
    });
    Ok(json!({ "sdk": SDK, "parse": report }))
}

/// Answers the SDK's requests on localhost:3001 (`BaseUrl::Localhost`): an empty spot meta for
/// `/info`, and for `/exchange` an error answer after keeping the body it was sent.
fn capture_server(captured: std::sync::Arc<std::sync::Mutex<Option<String>>>) -> Result<()> {
    use std::io::{BufRead, Read, Write};
    let listeners: Vec<std::net::TcpListener> =
        ["127.0.0.1:3001", "[::1]:3001"].iter().filter_map(|at| std::net::TcpListener::bind(at).ok()).collect();
    if listeners.is_empty() {
        return Err("localhost:3001 is in use".into());
    }
    for listener in listeners {
        let captured = std::sync::Arc::clone(&captured);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let captured = std::sync::Arc::clone(&captured);
                std::thread::spawn(move || {
                    let mut reader = std::io::BufReader::new(&stream);
                    loop {
                        let (mut path, mut length) = (String::new(), 0usize);
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        if let Some(target) = line.split_whitespace().nth(1) {
                            path = target.to_owned();
                        }
                        loop {
                            line.clear();
                            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                                return;
                            }
                            if line == "\r\n" {
                                break;
                            }
                            if let Some((name, value)) = line.split_once(':') {
                                if name.eq_ignore_ascii_case("content-length") {
                                    length = value.trim().parse().unwrap_or(0);
                                }
                            }
                        }
                        let mut body = vec![0; length];
                        if reader.read_exact(&mut body).is_err() {
                            return;
                        }
                        let answer = if path == "/exchange" {
                            if let Ok(mut captured) = captured.lock() {
                                *captured = Some(String::from_utf8_lossy(&body).into_owned());
                            }
                            r#"{"status":"err","response":"captured"}"#
                        } else {
                            r#"{"universe":[],"tokens":[]}"#
                        };
                        let reply = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{answer}",
                            answer.len()
                        );
                        if (&stream).write_all(reply.as_bytes()).is_err() {
                            return;
                        }
                    }
                });
            }
        });
    }
    Ok(())
}

/// Checks the copied private code against the SDK's own: the SDK's public `ExchangeClient::order`
/// posts the fixed order to a local server (`BaseUrl::Localhost`), and the copies in this file then
/// build, hash, sign and serialize the same order at the nonce the SDK chose. The two bodies must
/// be the same bytes. Nothing leaves the machine.
async fn verify_copy() -> Result<Value> {
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    capture_server(std::sync::Arc::clone(&captured))?;
    let fixed = fixed()?;
    let meta = Meta { universe: vec![AssetMeta { name: "BTC".to_string(), sz_decimals: 5 }] };
    let exchange = ExchangeClient::new(None, fixed.wallet.clone(), Some(BaseUrl::Localhost), Some(meta), None).await?;
    exchange.order(client_order(fixed.px, fixed.sz), None).await?;
    let sdk_body = captured.lock().map_err(|_| "capture lock")?.clone().ok_or("the SDK posted nothing")?;
    let nonce = serde_json::from_str::<Value>(&sdk_body)?["nonce"].as_u64().ok_or("no nonce in the SDK's body")?;
    // `BaseUrl::Localhost` is not mainnet, so the SDK signs with source "b"; the copy does the same.
    let action = convert(&fixed.template, &fixed.coin_to_asset, client_order(fixed.px, fixed.sz)).ok_or("no action")?;
    let connection_id = hash(&action, nonce, None).ok_or("hash failed")?;
    let signature = sign_l1_action(&fixed.wallet, connection_id, false).ok_or("signing failed")?;
    let payload = ExchangePayload { action: serde_json::to_value(&action)?, signature, nonce, vault_address: None };
    let copy_body = serde_json::to_string(&payload)?;
    Ok(json!({
        "sdk": SDK,
        "verify_copy": {
            "equal": copy_body == sdk_body,
            "bytes": sdk_body.len(),
            "sdk_body": sdk_body,
            "copy_body": copy_body,
        },
    }))
}

async fn serve(px: f64, sz: f64) -> Result<()> {
    // A fresh random key that has never held anything: the venue has no account for it.
    let wallet = LocalWallet::new(&mut ethers::core::rand::thread_rng());
    let address = wallet.address();
    let exchange = ExchangeClient::new(None, wallet, Some(BaseUrl::Mainnet), None, None).await?;
    let info = InfoClient::new(None, Some(BaseUrl::Mainnet)).await?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let answer = match line.trim() {
            "info" => {
                let start = Instant::now();
                let read = info.l2_snapshot("BTC".to_string()).await;
                let ns = start.elapsed().as_nanos() as u64;
                match read {
                    Ok(book) => {
                        let side = |at: usize| book.levels.get(at).map_or(0, Vec::len);
                        json!({"ok": true, "ns": ns, "answer": format!("{} bids {} asks", side(0), side(1))})
                    }
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "exchange" => {
                let start = Instant::now();
                let placed = exchange.order(client_order(px, sz), None).await;
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
    log::set_logger(&LOGGER).map_err(|error| error.to_string())?;
    log::set_max_level(log::LevelFilter::Error);
    let mut tally = LiveTally::default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut info = InfoClient::new(None, Some(BaseUrl::Mainnet)).await?;
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    // The SDK has no bbo subscription and no `fast` option for l2Book.
    info.subscribe(Subscription::L2Book { coin: "BTC".to_string() }, sender.clone()).await?;
    info.subscribe(Subscription::Trades { coin: "BTC".to_string() }, sender).await?;
    tally.connects += 1;
    while let Ok(Some(message)) = tokio::time::timeout_at(deadline, receiver.recv()).await {
        match message {
            Message::L2Book(book) => {
                tally.book_frames += 1;
                tally.aged(book.data.time);
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
    tally.unreadable = UNPARSED.load(Ordering::Relaxed);
    let mut report = tally.report(secs);
    report["bbo_frames"] = Value::Null;
    Ok(json!({ "sdk": SDK, "live": report }))
}
