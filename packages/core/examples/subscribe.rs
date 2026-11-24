//! Opens the mainnet websocket, subscribes to BTC's book and trades, prints the first ten updates
//! and exits: `cargo run -p df-hyperliquid-core --example subscribe`.

use std::error::Error;
use std::time::Duration;

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::native::Feed;
use df_hyperliquid_core::live::{Event, Frame, Session, Subscription};
use df_hyperliquid_core::markets::Side;
use tokio::time::timeout;

const UPDATES: usize = 10;
const WAIT: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut feed = Feed::start(Session::new(Network::Mainnet));
    let btc = || "BTC".to_owned();
    feed.subscribe(Subscription::L2Book { coin: btc(), n_sig_figs: None, mantissa: None, fast: false }).await?;
    feed.subscribe(Subscription::Trades { coin: btc() }).await?;

    let mut updates = 0;
    while updates < UPDATES {
        let event = timeout(WAIT, feed.next()).await?.ok_or("the feed stopped")?;
        match event {
            Event::Opened => println!("connected"),
            Event::Frame(frame) => match *frame {
                Frame::Book(book) => {
                    let (bid, ask) = book.touch();
                    let show = |price: Option<_>| price.map_or_else(|| "none".to_owned(), |price| format!("{price}"));
                    println!("book   {}  bid {}  ask {}", book.coin, show(bid), show(ask));
                    updates += 1;
                }
                Frame::Trades(trades) => {
                    for trade in &trades {
                        let side = match trade.side {
                            Side::Buy => "buy",
                            Side::Sell => "sell",
                        };
                        println!("trade  {}  {side} {} at {}", trade.coin, trade.sz, trade.px);
                    }
                    updates += 1;
                }
                Frame::Ack(ack) => println!("subscribed {}", ack.echo.compact()),
                _ => {}
            },
            Event::Unreadable(bad) => println!("unreadable frame: {bad}"),
            Event::Closed { reason, retry } => println!("closed ({reason}), dialling again in {} ms", retry.after_ms),
            _ => {}
        }
    }
    feed.close();
    Ok(())
}
