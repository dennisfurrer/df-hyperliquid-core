//! The first reads most programs need, from mainnet, with no key: what is listed, every mid, one
//! coin's book and last hour of candles, and, for the address in `HL_ADDRESS`, its account value,
//! positions, spot balances and open orders. Nothing is signed or sent.
//!
//! Environment: `HL_COIN` (default `BTC`), `HL_ADDRESS` (optional, any account's address).
//!
//! `cargo run -p df-hyperliquid-core --release --example quickstart`
//! `HL_ADDRESS=0x... cargo run -p df-hyperliquid-core --release --example quickstart`

use std::env;
use std::error::Error;

use df_hyperliquid_core::account::{self, clearinghouse_state, spot_state};
use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::markets::candle::{Interval, candle_snapshot};
use df_hyperliquid_core::markets::{Dex, Precision, book, mids, perps};
use df_hyperliquid_core::{Address, Client, Network};

/// One hour, in milliseconds.
const HOUR_MS: u64 = 3_600_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let coin = env::var("HL_COIN").unwrap_or_else(|_| "BTC".to_owned());
    let client = Client::reader(Native::new(), Network::Mainnet, clock);

    // What is listed on the main perp dex, and the coin's place in it.
    let meta = perps::meta(&client, Dex::Main).await?.ok_or("no main dex")?;
    let listed = meta.perps(0).count();
    let (asset, perp) = meta.find(0, &coin).ok_or_else(|| format!("{coin} is not listed"))?;
    println!(
        "{listed} perps on the main dex; {coin} is asset {}, {} size decimals, up to {}x",
        asset.id(),
        perp.sz_decimals,
        perp.max_leverage
    );

    // Every mid, in one read.
    let all = mids::all_mids(&client, Dex::Main).await?;
    println!("{} mids; {coin} {}", all.len(), all.get(&coin).map_or_else(|| "none".to_owned(), |mid| mid.to_string()));

    // The coin's book, best five levels a side.
    let levels = book::l2_book(&client, &coin, Precision::FULL).await?;
    println!("\n{coin} book, mid {}", levels.mid().map_or_else(|| "none".to_owned(), |mid| mid.to_string()));
    for (bid, ask) in levels.bids.iter().zip(levels.asks.iter()).take(5) {
        println!("  {:>12} {:>14}   |   {:<14} {:<12}", bid.sz, bid.px, ask.px, ask.sz);
    }

    // The last hour in five-minute candles.
    let now = clock();
    let candles = candle_snapshot(&client, &coin, Interval::FiveMinutes, now.saturating_sub(HOUR_MS), now).await?;
    println!("\n{coin}, last hour in 5 minute candles:");
    for candle in &candles {
        println!(
            "  open {:>12}  high {:>12}  low {:>12}  close {:>12}  volume {}",
            candle.open, candle.high, candle.low, candle.close, candle.volume
        );
    }

    // An account, read by address: no key involved.
    let Ok(text) = env::var("HL_ADDRESS") else {
        println!("\nset HL_ADDRESS to read an account's balances, positions and open orders");
        return Ok(());
    };
    let user: Address = text.parse().map_err(|error| format!("HL_ADDRESS: {error}"))?;
    let perps_state = clearinghouse_state(&client, user, Dex::Main).await?;
    println!("\naccount {user}");
    println!(
        "  perps account value {}, withdrawable {}",
        perps_state.margin_summary.account_value, perps_state.withdrawable
    );
    for held in &perps_state.asset_positions {
        let position = &held.position;
        let entry = position.entry_px.map_or_else(|| "none".to_owned(), |px| px.to_string());
        println!(
            "  position {} {}, entry {entry}, unrealized pnl {}",
            position.szi, position.coin, position.unrealized_pnl
        );
    }
    for balance in spot_state(&client, user).await?.balances.iter().filter(|balance| !balance.total.is_zero()) {
        println!("  spot {} {} ({} on hold)", balance.total, balance.coin, balance.hold);
    }
    let open = account::history::orders::open_orders(&client, user, Dex::Main).await?;
    println!("  {} open orders on the main dex", open.len());
    for order in open.iter().take(10) {
        println!("    {:?} {} {} at {}, oid {}", order.side, order.sz, order.coin, order.limit_px, order.oid);
    }
    Ok(())
}
