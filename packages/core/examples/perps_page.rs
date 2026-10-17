//! Reads what a BTC perp ticket needs from mainnet and prints the exact order a $20 market buy would send,
//! unsigned and unsent: `cargo run -p df-hyperliquid-core --example perps_page`.

use std::error::Error;

use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::markets::{self, Precision, perps};
use df_hyperliquid_core::trade::ticket::{Amount, Market, OrderIntent};
use df_hyperliquid_core::trade::{self, Side};
use df_hyperliquid_core::{Client, Dec, Network};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::reader(Native::new(), Network::Mainnet, clock);

    let meta = perps::meta_and_asset_ctxs(&client).await?;
    let (asset, perp, ctx) = meta.find("BTC").ok_or("BTC is not listed")?;
    let book = markets::reads::l2_book(&client, "BTC", Precision::FULL).await?;
    let (bid, ask) = book.touch();
    let mid = book.mid().ok_or("the book is empty")?;

    println!("BTC  asset {}  up to {}x  {} size decimals", asset.id(), perp.max_leverage, perp.sz_decimals);
    println!("     bid {}  ask {}  mid {mid}", show(bid), show(ask));
    println!("     mark {}  funding {}  open interest {}", ctx.mark_px, ctx.funding, ctx.open_interest);

    let market = Market::perp(asset, perp.sz_decimals, perp.max_leverage, perp.allows_cross()).ok_or("not a perp")?;
    let slippage: Dec = "0.01".parse()?;
    let intent = OrderIntent::market(Side::Buy, Amount::Notional("20".parse()?), mid, slippage);
    let order = trade::ticket::order(&market, &intent, None).map_err(|refusal| format!("{refusal:?}"))?;
    println!("\nwould send: {}", trade::L1Action::wire(&order));

    let spent = client.spent();
    println!("\nspent {} of the IP's {} a minute", spent.minute, spent.limit);
    Ok(())
}

fn show(price: Option<Dec>) -> String {
    price.map_or_else(|| "none".to_owned(), |price| price.to_string())
}
