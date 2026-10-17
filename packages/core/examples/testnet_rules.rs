//! Puts two HIP-4 rules no document settles to the testnet venue: the smallest outcome order it keeps
//! (`minimum`, resting `Alo` buys cancelled at once) and whether it refuses settling a question's fallback alone
//! (`fallback`, settled at 0 for good if it takes it). The key file holds a private key as hex, or `-` to only
//! read; every payload shows first and nothing is sent without `y`.
//!
//! ```sh
//! cargo run -p df-hyperliquid-core --example testnet_rules -- <key-file> minimum [outcome]
//! cargo run -p df-hyperliquid-core --example testnet_rules -- <key-file> fallback <question>
//! ```

use std::{
    env,
    error::Error,
    io::{self, Write},
};

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Status;
use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::deploy::outcomes::{Fraction, OutcomeDeploy, Venue};
use df_hyperliquid_core::markets::outcomes::{self, Coin, OutcomeMeta, OutcomeSide, Px, Side, Tif};
use df_hyperliquid_core::trade::{Cancel, L1Action, PlaceOrder};
use df_hyperliquid_core::{Client, Network};

const NETWORK: Network = Network::Testnet;

const USAGE: &str = "usage: testnet_rules <key-file | -> minimum [outcome] | fallback <question>";

/// A resting buy: its price, its size in contracts, and what it is worth at its price and at its
/// short side, the two ways the rules value it.
const CASES: [(&str, &str, &str); 6] = [
    ("0.02", "500", "10.00 at its price, 10.00 at its short side: both rules keep it"),
    ("0.02", "100", "2.00 at its price and its short side: a 10 USDC floor refuses, a 1.5 floor keeps"),
    ("0.02", "60", "1.20 at its price and its short side: under a 1.5 minimum, over a floor of 1"),
    ("0.02", "40", "0.80 at its price and its short side: under both"),
    ("0.70", "3", "2.10 at its price, 0.90 at its short side"),
    ("0.70", "2", "1.40 at its price, 0.60 at its short side"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (Some(key_file), Some(rule)) = (args.first(), args.get(1)) else { return Err(USAGE.into()) };
    let signer = if key_file == "-" { None } else { Some(key(key_file)?) };
    let reader = Client::reader(Native::new(), NETWORK, clock);
    let meta = outcomes::outcome_meta(&reader).await?;
    match rule.as_str() {
        "minimum" => {
            let outcome = args.get(2).map(|id| id.parse::<u32>()).transpose()?;
            minimum(&reader, &meta, outcome, signer).await
        }
        "fallback" => {
            let question = args.get(2).ok_or(USAGE)?.parse::<u32>()?;
            fallback(&meta, question, signer).await
        }
        _ => Err(USAGE.into()),
    }
}

async fn minimum(
    reader: &Client<Native>,
    meta: &OutcomeMeta,
    outcome: Option<u32>,
    signer: Option<LocalKey>,
) -> Result<(), Box<dyn Error>> {
    let coin = match outcome {
        Some(outcome) => Coin::new(outcome, OutcomeSide::Yes),
        None => roomy(reader, meta).await?.ok_or("no recent testnet outcome has room to rest a buy at 0.70")?,
    };
    let asset = coin.asset().ok_or("that outcome has no asset")?;
    let name = meta.outcome(coin.outcome()).map_or("not listed", |row| row.name.as_str());
    println!("outcome {} ({name}), Yes side, asset {}", coin.outcome(), asset.id());
    let client = signer.map(|key| Client::new(Native::new(), NETWORK, clock, key));
    if let Some(client) = &client {
        println!("signing as {}", client.signer().address());
    }
    let mut kept = Vec::new();
    for (price, size, worth) in CASES {
        let order = coin.limit(Side::Buy, price.parse()?, size.parse()?, Tif::Alo)?;
        let place = PlaceOrder::one(order);
        println!("\n{size} contracts at {price}: {worth}\n  {}", place.wire());
        let Some(client) = &client else { continue };
        if !confirm("  The buy, then its cancel if it rests.")? {
            kept.push(format!("{size} at {price}: not sent"));
            continue;
        }
        let said = match client.send(&place).await {
            Ok(accepted) => match accepted.statuses.first() {
                Some(Status::Ok(status)) => match status.get("resting").and_then(|resting| resting.get("oid")) {
                    Some(oid) => {
                        let oid = oid.as_u64().ok_or("the venue's oid is not a number")?;
                        let cancel = Cancel::one(asset, oid);
                        match client.send(&cancel).await {
                            Ok(_) => format!("kept (rested as {oid}, cancelled)"),
                            Err(error) => {
                                format!("kept (rested as {oid}; the cancel failed, cancel it by hand: {error})")
                            }
                        }
                    }
                    None => format!("kept, and did not rest: {status}"),
                },
                Some(Status::Error(reason)) => format!("refused: {reason}"),
                None => "answered with no status".to_owned(),
            },
            Err(error) => format!("refused: {error}"),
        };
        println!("  venue: {said}");
        kept.push(format!("{size} at {price} ({worth}): {said}"));
    }
    if client.is_some() {
        println!("\nwhat the venue did:");
        for line in kept {
            println!("  {line}");
        }
    }
    Ok(())
}

/// The newest outcome whose Yes side has no ask at or under 0.70, so every case rests rather
/// than being refused for crossing.
async fn roomy(reader: &Client<Native>, meta: &OutcomeMeta) -> Result<Option<Coin>, Box<dyn Error>> {
    let high: Px = "0.70".parse()?;
    let mut rows: Vec<u32> = meta.outcomes.iter().map(|row| row.outcome).collect();
    rows.sort_unstable_by(|a, b| b.cmp(a));
    for outcome in rows.into_iter().take(15) {
        let coin = Coin::new(outcome, OutcomeSide::Yes);
        let book = outcomes::book::book(reader, coin).await?;
        if book.asks.first().is_none_or(|level| level.px > high) {
            return Ok(Some(coin));
        }
    }
    Ok(None)
}

async fn fallback(meta: &OutcomeMeta, question: u32, signer: Option<LocalKey>) -> Result<(), Box<dyn Error>> {
    let listed = meta.question(question).ok_or("testnet lists no such question")?;
    let outcome = listed.fallback_outcome.ok_or("this question lists no fallback")?;
    let row = meta.outcome(outcome).ok_or("its fallback is not listed")?;
    let venue_name = row.venue.as_deref().ok_or("its fallback names no venue")?;
    let venue: Venue = venue_name.parse()?;
    let settlement = row.settlement(Fraction::ZERO).ok_or("its fallback does not list two sides")?;
    let action = OutcomeDeploy::settle_outcome(venue, settlement);
    println!("question {question} ({}), its fallback {outcome}, venue {venue_name}", listed.name);
    if let Some(deployer) = meta.deployer(venue_name) {
        println!("the venue's deployer: {}", deployer.deployer);
    }
    println!("settle the fallback alone, at 0:\n  {}", action.wire());
    let Some(key) = signer else { return Ok(()) };
    let client = Client::new(Native::new(), NETWORK, clock, key);
    println!("signing as {}", client.signer().address());
    if !confirm("  If the venue takes it, the fallback is settled at 0 for good.")? {
        return Ok(());
    }
    match client.send(&action).await {
        Ok(accepted) => {
            let refused: Vec<&str> = accepted.errors().collect();
            if refused.is_empty() {
                println!("  venue: took it ({})", accepted.kind);
            } else {
                println!("  venue: refused inside an ok: {}", refused.join("; "));
            }
        }
        Err(error) => println!("  venue: refused: {error}"),
    }
    Ok(())
}

/// The private key in `path`, as hex. It is never printed.
fn key(path: &str) -> Result<LocalKey, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(LocalKey::from_hex(text.trim())?)
}

fn confirm(what: &str) -> Result<bool, io::Error> {
    print!("{what} Send? [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().eq_ignore_ascii_case("y"))
}
