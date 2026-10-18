//! The native websocket feed as a user runs it: a [`Store`] says what it needs, a [`Feed`]
//! carries the session over a socket to a local server that plays the venue with frames recorded
//! from mainnet (`fixtures/ws`), and what comes back is folded into the store and read from its
//! views. The server checks every frame the feed sends, byte for byte.

mod server;

use std::time::Duration;

use df_hyperliquid_core::account::{Abstraction, ClearinghouseState, L1Action, Leverage, Signature, recover};
use df_hyperliquid_core::client::ExchangeResponse;
use df_hyperliquid_core::client::native::Feed;
use df_hyperliquid_core::live::store::{Folded, Interest, Read, Store};
use df_hyperliquid_core::live::{Answer, Event, Frame, Post, Session, Timing};
use df_hyperliquid_core::markets::Level;
use df_hyperliquid_core::trade::{self, Amount, Asset, Market, OrderAnswer, OrderIntent, Side, Tif};
use df_hyperliquid_core::utils::sign::l1_digest;
use df_hyperliquid_core::utils::{Json, hex};
use df_hyperliquid_core::{Address, Dec, Network};
use futures_util::StreamExt;
use tokio::time::timeout;

use crate::support::{NOW, fixture, key};
use server::{accept, listen};

/// The account the 2026-10-08 frames follow, and the one the 2026-09-26 frames follow.
const TRADER: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";
const HOLDER: &str = "0x8d1548a936241928cef288a1c377919a5d511f7a";

const BOOK: &str = r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"BTC"}}"#;
const TRADES: &str = r#"{"method":"subscribe","subscription":{"type":"trades","coin":"BTC"}}"#;

/// Reconnects within a few dozen milliseconds, and no ping for as long as a test runs.
const QUICK: Timing =
    Timing { ping_after_ms: 30_000, silent_ms: 60_000, steady_ms: 30_000, first_retry_ms: 20, last_retry_ms: 40 };

const WAIT: Duration = Duration::from_secs(5);

/// A frame from a recording under `fixtures/ws`, as received.
fn recorded(manifest: &str, name: &str) -> String {
    let file: serde_json::Value = serde_json::from_str(&fixture(&format!("ws/{manifest}.json"))).unwrap();
    file["frames"][name].as_str().unwrap_or_else(|| panic!("ws/{manifest}.json has no frame {name}")).to_owned()
}

fn mainnet(name: &str) -> String {
    recorded("mainnet", name)
}

/// A recorded answer to a post, carrying the id the post went out with, as the venue puts it back.
fn answering(text: &str, id: u64) -> String {
    let mut frame: serde_json::Value = serde_json::from_str(text).unwrap();
    frame["data"]["id"] = id.into();
    frame.to_string()
}

async fn event(feed: &mut Feed) -> Event {
    timeout(WAIT, feed.next()).await.expect("an event in time").expect("a running feed")
}

/// The next frame, past the socket opening.
async fn frame(feed: &mut Feed) -> Frame {
    loop {
        match event(feed).await {
            Event::Opened => {}
            Event::Frame(frame) => return *frame,
            other => panic!("expected a frame, got {other:?}"),
        }
    }
}

/// What the store's interests changed since the socket was last told, told to the feed.
async fn tell(store: &mut Store, feed: &Feed) {
    let changes = store.changes();
    for subscription in changes.unsubscribe {
        feed.unsubscribe(subscription).unwrap();
    }
    for subscription in changes.subscribe {
        feed.subscribe(subscription).await.unwrap();
    }
}

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[tokio::test]
async fn a_book_and_the_mids_subscribed_through_the_feed_are_read_from_the_store() {
    let (listener, url) = listen().await;
    let mut store = Store::new(Network::Mainnet);
    let book = store.register(Interest::Book { coin: "BTC".to_owned() }).unwrap();
    let _mids = store.register(Interest::Mids { dex: String::new() }).unwrap();
    let mut session = Session::new(Network::Mainnet);
    assert!(store.sync(&mut session).unwrap().frames.is_empty(), "nothing goes out before the socket opens");
    let mut feed = Feed::start_at(url, session);

    let mut venue = accept(&listener).await;
    assert_eq!(venue.takes().await, BOOK);
    assert_eq!(venue.takes().await, r#"{"method":"subscribe","subscription":{"type":"allMids"}}"#);
    for name in ["ack.l2Book", "ack.allMids", "l2Book", "allMids"] {
        venue.sends(&mainnet(name)).await;
    }
    let mut folded = Vec::new();
    for _ in 0..4 {
        folded.push(store.apply(NOW, &frame(&mut feed).await));
    }
    assert_eq!(folded, [Folded::Dropped, Folded::Dropped, Folded::Changed, Folded::Changed], "acks are not state");

    let level = |px: &str, sz: &str, n: u32| Level { px: dec(px), sz: dec(sz), n };
    assert_eq!(store.best("BTC"), (Some(level("80459.0", "16.87185", 49)), Some(level("80460.0", "4.21967", 21))));
    assert_eq!(store.book("BTC").map(|book| (book.time, book.heard)), Some((1_791_480_444_439, NOW)));
    assert_eq!(store.mid("BTC"), Some(dec("80459.5")), "halfway across the held book");
    assert_eq!(store.mid("@107"), Some(dec("82.897")), "a spot pair from the first dex's mids");
    assert_eq!(store.mid("#14720"), Some(dec("0.5")), "and an outcome side");

    drop(book);
    tell(&mut store, &feed).await;
    assert_eq!(venue.takes().await, r#"{"method":"unsubscribe","subscription":{"type":"l2Book","coin":"BTC"}}"#);
    assert!(store.book("BTC").is_none());
    assert_eq!(store.mid("BTC"), Some(dec("80474.5")), "the venue's own mid once no book is held");
}

#[tokio::test]
async fn two_accounts_clearinghouses_orders_and_fills_fold_into_their_views() {
    let (listener, url) = listen().await;
    let (trader, holder) = (address(TRADER), address(HOLDER));
    let mut store = Store::new(Network::Mainnet);
    let _held = [
        Interest::State { user: trader },
        Interest::Orders { user: trader, dex: String::new() },
        Interest::Fills { user: trader },
        Interest::State { user: holder },
    ]
    .map(|interest| store.register(interest).unwrap());
    let mut session = Session::new(Network::Mainnet);
    store.sync(&mut session).unwrap();
    let mut feed = Feed::start_at(url, session);

    let mut venue = accept(&listener).await;
    let subscribe = |subscription: String| format!(r#"{{"method":"subscribe","subscription":{subscription}}}"#);
    let mut asked = Vec::new();
    for _ in 0..8 {
        asked.push(venue.takes().await);
    }
    let mut wanted = Vec::new();
    for user in [TRADER, HOLDER] {
        wanted.push(subscribe(format!(r#"{{"type":"allDexsClearinghouseState","user":"{user}"}}"#)));
        wanted.push(subscribe(format!(r#"{{"type":"spotState","user":"{user}"}}"#)));
        wanted.push(subscribe(format!(r#"{{"type":"webData3","user":"{user}"}}"#)));
        if user == TRADER {
            wanted.push(subscribe(format!(r#"{{"type":"openOrders","user":"{user}","dex":""}}"#)));
            wanted.push(subscribe(format!(r#"{{"type":"userFills","user":"{user}"}}"#)));
        }
    }
    assert_eq!(asked, wanted, "each account's state, then the trader's orders and fills, in the order wanted");

    let sent = [
        mainnet("allDexsClearinghouseState"),
        mainnet("webData3"),
        mainnet("openOrders"),
        mainnet("userFills.liquidation"),
        mainnet("userFills"),
        recorded("portfolio", "allDexsClearinghouseState"),
    ];
    for text in &sent {
        venue.sends(text).await;
    }
    for text in &sent {
        assert_eq!(store.apply(NOW, &frame(&mut feed).await), Folded::Changed, "{}", &text[..60]);
    }

    let account = store.account(trader).unwrap();
    assert_eq!(account.dexes().count(), 11);
    assert_eq!(account.mode().map(|mode| mode.value.clone()), Some(Abstraction::Disabled));
    let positions: Vec<(&str, Dec)> =
        store.positions(trader).iter().map(|position| (position.coin.as_str(), position.szi)).collect();
    assert_eq!(positions, [("BTC", dec("-12.07087"))]);
    let orders: Vec<(u64, Dec)> = store.orders(trader).iter().map(|order| (order.oid, order.limit_px)).collect();
    assert_eq!(orders, [(569_017_061_987, dec("80459.0"))]);
    let fills = account.fills().unwrap();
    assert_eq!((fills.len(), fills[0].tid), (31, 167_376_889_247_413), "the snapshot, then the live fill on top");
    assert_eq!(fills.iter().filter(|fill| fill.liquidation.is_some()).count(), 3);
    assert_eq!(
        store.positions(holder)[0].leverage,
        Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") },
        "the other account's isolated long, with the margin put up"
    );
    assert_eq!(store.followed(), [trader, holder]);

    venue.sends(&mainnet("openOrders.empty")).await;
    assert_eq!(store.apply(NOW + 1_000, &frame(&mut feed).await), Folded::Changed);
    assert!(store.orders(trader).is_empty(), "the order rests no more");
}

#[tokio::test]
async fn a_dropped_socket_is_dialled_again_and_resubscribed_and_a_post_cut_off_is_reported() {
    let (listener, url) = listen().await;
    let mut store = Store::new(Network::Mainnet);
    let _book = store.register(Interest::Book { coin: "BTC".to_owned() }).unwrap();
    let mut session = Session::new(Network::Mainnet).with_timing(QUICK);
    store.sync(&mut session).unwrap();
    let mut feed = Feed::start_at(url, session);

    let mut first = accept(&listener).await;
    assert_eq!(first.takes().await, BOOK);
    first.sends(&mainnet("l2Book")).await;
    assert_eq!(store.apply(NOW, &frame(&mut feed).await), Folded::Changed);
    let _tape = store.register(Interest::Trades { coin: "BTC".to_owned() }).unwrap();
    tell(&mut store, &feed).await;
    assert_eq!(first.takes().await, TRADES, "a new interest goes out on the open socket at once");
    let read = Post::info(r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}"#).unwrap();
    let id = feed.post(read).await.unwrap();
    assert_eq!(
        first.takes().await,
        r#"{"method":"post","id":1,"request":{"type":"info","payload":{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}}}"#
    );
    first.0.close(None).await.unwrap();

    let Event::Closed { retry, .. } = event(&mut feed).await else { panic!("the drop is reported") };
    assert_eq!(retry.unanswered, [id], "unknown, not failed: never sent again");
    assert!((20..=25).contains(&retry.after_ms), "{}", retry.after_ms);

    let mut second = accept(&listener).await;
    assert_eq!(second.takes().await, BOOK);
    assert_eq!(second.takes().await, TRADES, "everything wanted, in the order first wanted");
    second.sends(&mainnet("trades")).await;
    assert_eq!(store.apply(NOW + 1_000, &frame(&mut feed).await), Folded::Changed);
    assert!(timeout(Duration::from_millis(300), second.0.next()).await.is_err(), "and the post is not among them");

    assert_eq!(store.book("BTC").map(|book| book.heard), Some(NOW), "what was held stays across the drop");
    let prints: Vec<u64> = store.trades("BTC").unwrap().iter().map(|print| print.tid).collect();
    assert_eq!(prints, [8_986_569_314_831, 717_978_735_161_105]);
}

#[tokio::test]
async fn a_read_and_a_signed_order_posted_on_the_socket_are_answered_by_their_ids() {
    let (listener, url) = listen().await;
    let trader = address(TRADER);
    let mut store = Store::new(Network::Mainnet);
    let _state = store.register(Interest::State { user: trader }).unwrap();
    let mut feed = Feed::start_at(url, Session::new(Network::Mainnet));
    let mut venue = accept(&listener).await;
    assert_eq!(event(&mut feed).await, Event::Opened);

    let read = Post::info(&format!(r#"{{"type":"clearinghouseState","user":"{TRADER}"}}"#)).unwrap();
    let id = feed.post(read).await.unwrap();
    assert_eq!(
        venue.takes().await,
        format!(
            r#"{{"method":"post","id":1,"request":{{"type":"info","payload":{{"type":"clearinghouseState","user":"{TRADER}"}}}}}}"#
        )
    );
    venue.sends(&answering(&mainnet("post.info.clearinghouseState"), id)).await;
    let Frame::Post(reply) = frame(&mut feed).await else { panic!("an answer") };
    let Answer::Info { kind, data } = reply.answer else { panic!("an info answer") };
    assert_eq!((reply.id, kind.as_str()), (id, "clearinghouseState"));
    let state: ClearinghouseState = serde_json::from_str(&data).unwrap();
    assert_eq!(store.read(NOW, Read::Clearinghouse { user: trader, dex: "", state: &state }), Folded::Changed);
    assert_eq!(store.positions(trader)[0].szi, dec("-12.45034"));

    // The post-requests page's example order: 0.2 ETH (asset 4) at 1100, good till canceled.
    let eth = Market::perp(Asset::perp(4).unwrap(), 4, 25, true).unwrap();
    let intent = OrderIntent::limit(Side::Buy, Amount::Size(dec("0.2")), dec("1100"), Tif::Gtc);
    let order = trade::orders::order(&eth, &intent, None).unwrap();
    let signature = key().sign_digest(&order.digest(NOW, None, Network::Mainnet)).unwrap();
    let id = feed.post(Post::action(&order.body(NOW, &signature, None)).unwrap()).await.unwrap();
    let sent = Json::parse(&venue.takes().await).unwrap();
    assert_eq!(
        (sent.get("method").and_then(Json::as_str), sent.get("id").and_then(Json::as_u64)),
        (Some("post"), Some(2))
    );
    let request = sent.get("request").unwrap();
    assert_eq!(request.get("type").and_then(Json::as_str), Some("action"));
    let payload = request.get("payload").unwrap();
    let action = payload.get("action").unwrap();
    assert_eq!(
        action.compact(),
        r#"{"type":"order","orders":[{"a":4,"b":true,"p":"1100","s":"0.2","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#
    );
    assert_eq!(
        (payload.get("nonce").and_then(Json::as_u64), payload.get("vaultAddress")),
        (Some(NOW), Some(&Json::Null))
    );
    let word = |key: &str| -> [u8; 32] {
        hex::decode_array(
            payload.get("signature").and_then(|signature| signature.get(key)).and_then(Json::as_str).unwrap(),
        )
        .unwrap()
    };
    let v = payload.get("signature").and_then(|signature| signature.get("v")).and_then(Json::as_u64).unwrap();
    let posted = Signature::from_parts(word("r"), word("s"), u8::try_from(v).unwrap());
    let digest = l1_digest(&action.to_action().unwrap().to_msgpack(), NOW, None, Network::Mainnet);
    assert_eq!(recover(&digest, &posted), Ok(key().address()), "the action as posted recovers to the signer");

    venue.sends(&answering(&recorded("docs", "post.action.ok"), id)).await;
    let Frame::Post(reply) = frame(&mut feed).await else { panic!("an answer") };
    let Answer::Action(ExchangeResponse::Ok(accepted)) = reply.answer else { panic!("an accepted action") };
    assert_eq!(reply.id, id);
    assert_eq!(trade::answer::orders(&accepted, 1), Ok(vec![OrderAnswer::Resting { oid: 88_383, cloid: None }]));
}
