//! Lists the HIP-4 outcome markets on mainnet with their titles, picks the first whose Yes side
//! has a bid, and signs a post-only buy of Yes at half that bid, so it rests and does not fill, and
//! the cancel for it, with the key in `HL_PRIVATE_KEY`. By default it prints both signed bodies and
//! sends nothing; with `HL_SEND=1` it places the order and then cancels it:
//! `HL_PRIVATE_KEY=0x... cargo run -p df-hyperliquid-core --example outcome_trade`.

use std::env;
use std::error::Error;

use df_hyperliquid_core::account::{LocalKey, SignError};
use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::markets::outcomes::{
    self, Fields, Outcome, OutcomeTemplate, Px, Qty, fill, label, outcome_meta, outcome_templates,
};
use df_hyperliquid_core::markets::{Precision, book};
use df_hyperliquid_core::trade::answer::{cancels, orders};
use df_hyperliquid_core::trade::{CancelByCloid, Cloid, L1Action, PlaceOrder, Side, Tif};
use df_hyperliquid_core::utils::OutcomeSide;
use df_hyperliquid_core::{Client, Network};
use zeroize::Zeroizing;

/// How many outcomes to list, and how many books to read looking for a bid.
const LISTED: usize = 10;
const TRIED: usize = 20;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let secret = Zeroizing::new(env::var("HL_PRIVATE_KEY").map_err(|_| "set HL_PRIVATE_KEY to the signing key")?);
    let key = LocalKey::from_hex(&secret)?;
    let send = env::var("HL_SEND").is_ok_and(|value| value == "1");
    let client = Client::new(Native::new(), Network::Mainnet, clock, key);
    println!("signer {}", client.signer().address());

    let meta = outcome_meta(&client).await?;
    let templates = outcome_templates(&client).await?;
    println!("{} outcomes in {} questions", meta.outcomes.len(), meta.questions.len());
    for outcome in meta.outcomes.iter().take(LISTED) {
        let fields = Fields::parse(&outcome.description);
        let sides: Vec<String> = outcome.side_specs.iter().map(|side| label(Some(&side.name), &fields, "?")).collect();
        println!("  {}  {}  ({})", outcome.outcome, title(&templates, outcome), sides.join(" / "));
    }

    let mut picked = None;
    for outcome in meta.outcomes.iter().take(TRIED) {
        let yes = outcome.coin(OutcomeSide::Yes);
        let book = outcomes::Book::try_from(book::l2_book(&client, &yes.to_string(), Precision::FULL).await?)?;
        if let Some(bid) = book.bids.first() {
            picked = Some((outcome, yes, bid.px));
            break;
        }
    }
    let (outcome, yes, bid) = picked.ok_or("no outcome among those read has a bid")?;

    // Half the best bid, on the grid, and the fewest contracts worth the minimum of 10 there.
    let price = Px::saturating_from_ticks(bid.ticks() / 2).clamp_to_quotable();
    let size = Qty::least_at(price).ok_or("no size at a zero price")?;
    let cloid = Cloid::from_u128(u128::from(clock()));
    let order = yes.order(Side::Buy, price, size, Tif::Alo)?.with_cloid(cloid);
    let place = PlaceOrder::one(order);
    let cancel = CancelByCloid::one(order.asset(), cloid);
    println!("\npicked {} ({yes}), best bid {bid}", title(&templates, outcome));
    println!("buy {size} Yes at {price}, worth {}, client id {cloid}", size.notional(price));

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

/// The outcome's title: its template's pattern filled from its description, or its name as listed.
fn title(templates: &[OutcomeTemplate], outcome: &Outcome) -> String {
    let id = outcome.name.strip_prefix("template:");
    match templates.iter().find(|template| Some(template.id.as_str()) == id) {
        Some(template) => {
            let fields = Fields::parse(&outcome.description);
            fill(&template.name, |key| fields.text(key))
        }
        None => outcome.name.clone(),
    }
}

/// The body `/exchange` would take for `action`, signed at the client's next nonce.
fn signed(client: &Client<Native, LocalKey>, action: &impl L1Action) -> Result<String, SignError> {
    let nonce = client.next_nonce();
    let signature = client.signer().sign_digest(&action.digest(nonce, None, client.network()))?;
    Ok(action.body(nonce, &signature, None))
}
