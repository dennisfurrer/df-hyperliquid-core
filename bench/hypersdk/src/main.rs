//! hypersdk benchmarks, the same subcommands and output as `bench-df`:
//! `sign-check`, `sign`, `parse --corpus <path>`, `serve --px <price> --sz <size>` and
//! `live --secs <n>`. Each call is the one hypersdk's README and docs show.

use std::error::Error;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bench_common::{
    CountingAlloc,
    FIXED_ASSET, FIXED_KEY, FIXED_NONCE, FIXED_PX, FIXED_SZ, LiveTally, args, flag,
    load_corpus, parse_report, sign_samples, sign_warmup, summary_ns, time_each, warm,
};
use futures::StreamExt;
use hypersdk::Decimal;
use hypersdk::hypercore::types::api::Action;
use hypersdk::hypercore::types::{
    BatchOrder, Incoming, OrderGrouping, OrderRequest, OrderTypePlacement, Subscription, TimeInForce,
};
use hypersdk::hypercore::ws::Event;
use hypersdk::hypercore::{self, Chain, PrivateKeySigner, Signature};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

/// hypersdk drops a websocket frame it cannot read and logs `unable to parse` at warn level.
/// This logger only counts those lines, so the live run can report them.
struct CountUnparsed;

static UNPARSED: AtomicU64 = AtomicU64::new(0);

impl log::Log for CountUnparsed {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.level() == log::Level::Warn && record.args().to_string().starts_with("unable to parse") {
            UNPARSED.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

static LOGGER: CountUnparsed = CountUnparsed;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args();
    let report = match args.first().map(String::as_str) {
        Some("sign-check") => sign_check()?,
        Some("sign") => sign()?,
        Some("parse") => parse(&flag(&args, "--corpus").ok_or("--corpus <path>")?)?,
        Some("serve") => {
            let px = Decimal::from_str(&flag(&args, "--px").ok_or("--px <price>")?)?;
            let sz = Decimal::from_str(&flag(&args, "--sz").ok_or("--sz <size>")?)?;
            return serve(px, sz).await;
        }
        Some("live") => live(flag(&args, "--secs").ok_or("--secs <n>")?.parse()?).await?,
        _ => return Err("usage: bench-hypersdk sign-check | sign | parse | serve | live".into()),
    };
    println!("{report}");
    Ok(())
}

fn order(px: Decimal, sz: Decimal) -> BatchOrder {
    BatchOrder {
        orders: vec![OrderRequest {
            asset: FIXED_ASSET as usize,
            is_buy: true,
            limit_px: px,
            sz,
            reduce_only: false,
            order_type: OrderTypePlacement::Limit { tif: TimeInForce::Alo },
            cloid: Default::default(),
        }],
        grouping: OrderGrouping::Na,
        builder: None,
    }
}

fn fixed() -> Result<(PrivateKeySigner, Decimal, Decimal)> {
    let signer: PrivateKeySigner = FIXED_KEY.parse()?;
    Ok((signer, Decimal::from_str(FIXED_PX)?, Decimal::from_str(FIXED_SZ)?))
}

fn hex65(signature: &Signature) -> String {
    format!("0x{:064x}{:064x}{:02x}", signature.r, signature.s, signature.v)
}

fn sign_check() -> Result<Value> {
    let (signer, px, sz) = fixed()?;
    let request = Action::Order(order(px, sz)).sign_sync(&signer, FIXED_NONCE, None, None, Chain::Mainnet)?;
    Ok(json!({
        "sdk": "hypersdk",
        "address": format!("{:#x}", signer.address()),
        "signature": hex65(&request.signature),
        "action": serde_json::to_value(&request.action)?,
    }))
}

fn sign() -> Result<Value> {
    let (signer, px, sz) = fixed()?;
    let action = Action::Order(order(px, sz));

    // Hash and sign an order action that is already built. `sign_sync` takes the action by
    // value, so each call signs a clone of it.
    let hash_and_sign = || action.clone().sign_sync(&signer, FIXED_NONCE, None, None, Chain::Mainnet).ok();
    warm(sign_warmup(), hash_and_sign);
    let signed = time_each(sign_samples(), hash_and_sign);

    // From typed price and size to the JSON body `/exchange` takes.
    let build_sign_body = || -> Option<Vec<u8>> {
        let request = Action::Order(order(px, sz)).sign_sync(&signer, FIXED_NONCE, None, None, Chain::Mainnet).ok()?;
        serde_json::to_vec(&request).ok()
    };
    warm(sign_warmup(), build_sign_body);
    let bodies = time_each(sign_samples(), build_sign_body);

    // The clone alone, so the fairness notes can say what it adds to hash + sign.
    let cloned = || action.clone();
    warm(sign_warmup(), cloned);
    let clones = time_each(sign_samples(), cloned);

    Ok(json!({
        "sdk": "hypersdk",
        "sign": summary_ns(&signed),
        "build_sign_body": summary_ns(&bodies),
        "clone_action": summary_ns(&clones),
    }))
}

fn parse(path: &str) -> Result<Value> {
    let corpus = load_corpus(path)?;
    // What hypersdk's websocket stream does with each text frame.
    let report = parse_report(&corpus, |text| serde_json::from_slice::<Incoming>(text.as_bytes()).is_ok());
    Ok(json!({ "sdk": "hypersdk", "parse": report }))
}

async fn serve(px: Decimal, sz: Decimal) -> Result<()> {
    // A fresh random key that has never held anything: the venue has no account for it.
    let signer = PrivateKeySigner::random();
    let client = hypercore::mainnet();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let answer = match line.trim() {
            "info" => {
                let start = Instant::now();
                let read = client.l2_book("BTC".into(), None, None).await;
                let ns = start.elapsed().as_nanos() as u64;
                match read {
                    Ok(book) => {
                        json!({"ok": true, "ns": ns, "answer": format!("{} bids {} asks", book.levels[0].len(), book.levels[1].len())})
                    }
                    Err(error) => json!({"ok": false, "ns": ns, "answer": error.to_string()}),
                }
            }
            "exchange" => {
                let start = Instant::now();
                let nonce = bench_common::now_ms();
                let placed = client.place(&signer, order(px, sz), nonce, None, None).await;
                let ns = start.elapsed().as_nanos() as u64;
                match placed {
                    Ok(statuses) => json!({"ok": true, "ns": ns, "answer": format!("accepted: {statuses:?}")}),
                    // hypersdk folds a venue refusal and a transport failure into one error
                    // string; a venue refusal is the expected answer here.
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

async fn live(secs: u64) -> Result<Value> {
    log::set_logger(&LOGGER).map_err(|error| error.to_string())?;
    log::set_max_level(log::LevelFilter::Warn);
    let mut tally = LiveTally::default();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ws = hypercore::mainnet_ws();
    ws.subscribe(Subscription::L2Book { coin: "BTC".into(), n_sig_figs: None, mantissa: None, fast: true });
    ws.subscribe(Subscription::Bbo { coin: "BTC".into() });
    ws.subscribe(Subscription::Trades { coin: "BTC".into() });
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
        match event {
            Event::Connected => tally.connects += 1,
            Event::Disconnected => {}
            Event::Message(Incoming::L2Book(book)) => {
                tally.book_frames += 1;
                tally.aged(book.time);
            }
            Event::Message(Incoming::Bbo(bbo)) => {
                tally.bbo_frames += 1;
                tally.aged(bbo.time);
            }
            Event::Message(Incoming::Trades(trades)) => {
                tally.trade_frames += 1;
                tally.trades += trades.len() as u64;
                if let Some(newest) = trades.iter().map(|trade| trade.time).max() {
                    tally.aged(newest);
                }
            }
            Event::Message(_) => tally.other_frames += 1,
        }
    }
    tally.unreadable = UNPARSED.load(Ordering::Relaxed);
    Ok(json!({ "sdk": "hypersdk", "live": tally.report(secs) }))
}
