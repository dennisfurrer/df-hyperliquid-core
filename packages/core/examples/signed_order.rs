//! Signs a post-only BTC limit buy 30% under the mid, so it rests and does not fill, and the cancel
//! for it, with the key in `HL_PRIVATE_KEY`. By default it prints both signed bodies and sends
//! nothing; with `HL_SEND=1` it places the order on mainnet and then cancels it:
//! `HL_PRIVATE_KEY=0x... cargo run -p df-hyperliquid-core --example signed_order`.

use std::env;
use std::error::Error;

use df_hyperliquid_core::account::{LocalKey, SignError};
use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::markets::{self, Dex, Precision, perps};
use df_hyperliquid_core::trade::answer::{cancels, orders};
use df_hyperliquid_core::trade::{self, Amount, CancelByCloid, Cloid, L1Action, Market, OrderIntent, Side, Tif};
use df_hyperliquid_core::{Client, Dec, Network};
use zeroize::Zeroizing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let secret = Zeroizing::new(env::var("HL_PRIVATE_KEY").map_err(|_| "set HL_PRIVATE_KEY to the signing key")?);
    let key = LocalKey::from_hex(&secret)?;
    let send = env::var("HL_SEND").is_ok_and(|value| value == "1");
    let client = Client::new(Native::new(), Network::Mainnet, clock, key);
    println!("signer {}", client.signer().address());

    let meta = perps::meta(&client, Dex::Main).await?.ok_or("no main dex")?;
    let (asset, btc) = meta.find(0, "BTC").ok_or("BTC is not listed")?;
    let market = Market::perp(asset, btc.sz_decimals, btc.max_leverage, btc.allows_cross()).ok_or("not a perp")?;
    let book = markets::book::l2_book(&client, "BTC", Precision::FULL).await?;
    let mid = book.mid().ok_or("the book is empty")?;

    // 30% under the mid, worth $12. Alo is refused rather than filled if it would cross.
    let price = mid.checked_mul("0.7".parse()?).ok_or("the price overflowed")?;
    let notional: Dec = "12".parse()?;
    let cloid = Cloid::from_u128(u128::from(clock()));
    let intent = OrderIntent::limit(Side::Buy, Amount::Notional(notional), price, Tif::Alo).with_cloid(cloid);
    let place = trade::orders::order(&market, &intent, None)?;
    let cancel = CancelByCloid::one(market.asset(), cloid);
    let order = place.orders().first().ok_or("no order")?;
    println!("BTC mid {mid}: buy {} at {}, client id {cloid}", order.size(), order.price());

    if !send {
        println!("\norder body: {}", signed(&client, &place)?);
        println!("\ncancel body: {}", signed(&client, &cancel)?);
        println!("\ndry run: nothing was sent. Set HL_SEND=1 to place and cancel the order.");
        return Ok(());
    }

    let placed = client.send(&place).await?;
    let answer = orders(&placed, 1)?.into_iter().next().ok_or("no answer for the order")?;
    println!("\norder: {answer:?}");
    if answer.oid().is_none() {
        return Ok(());
    }
    let cancelled = client.send(&cancel).await?;
    println!("cancel: {:?}", cancels(&cancelled, 1)?);
    Ok(())
}

/// The body `/exchange` would take for `action`, signed at the client's next nonce.
fn signed(client: &Client<Native, LocalKey>, action: &impl L1Action) -> Result<String, SignError> {
    let nonce = client.next_nonce();
    let signature = client.signer().sign_digest(&action.digest(nonce, None, client.network()))?;
    Ok(action.body(nonce, &signature, None))
}
