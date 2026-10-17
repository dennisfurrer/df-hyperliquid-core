//! Against a websocket server on 127.0.0.1. Its frames were recorded from
//! wss://api.hyperliquid.xyz/ws on 2026-10-08 (`fixtures/ws`); the server only
//! puts back the id a post went out with.

use std::time::Duration;

use crate::client::Network;
use crate::live::{Frame, Post, Refused, Session, Subscription, Timing};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

use super::*;

const EMPTY_BOOK: &str = r##"{"channel":"l2Book","data":{"coin":"#14720","time":1791480447083,"levels":[[],[]]}}"##;
const PONG: &str = r#"{"channel":"pong"}"#;
const NULL_ANSWER: &str =
    r#"{"channel":"post","data":{"id":11,"response":{"type":"info","payload":{"type":"l2Book","data":null}}}}"#;

/// Quick enough for a test to see every rule fire.
const FAST: Timing =
    Timing { ping_after_ms: 150, silent_ms: 400, steady_ms: 10_000, first_retry_ms: 20, last_retry_ms: 40 };

const WAIT: Duration = Duration::from_secs(5);

fn book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}

async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

async fn accept(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    accept_async(stream).await.unwrap()
}

async fn text(socket: &mut WebSocketStream<TcpStream>) -> String {
    loop {
        match timeout(WAIT, socket.next()).await.unwrap().unwrap().unwrap() {
            Message::Text(text) => return text.to_string(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected text, got {other:?}"),
        }
    }
}

async fn event(feed: &mut Feed) -> Event {
    timeout(WAIT, feed.next()).await.unwrap().unwrap()
}

#[tokio::test]
async fn it_subscribes_on_open_and_hands_back_typed_frames() {
    let (listener, url) = listen().await;
    let feed_session = Session::new(Network::Mainnet);
    let mut feed = Feed::start_at(url, feed_session);
    feed.subscribe(book("#14720")).await.unwrap();
    let mut server = accept(&listener).await;
    assert_eq!(text(&mut server).await, crate::live::subscribe(&book("#14720")));
    server.send(Message::text(EMPTY_BOOK)).await.unwrap();
    assert_eq!(event(&mut feed).await, Event::Opened);
    match event(&mut feed).await {
        Event::Frame(frame) => match *frame {
            Frame::Book(book) => assert_eq!((book.coin.as_str(), book.bids.len()), ("#14720", 0)),
            other => panic!("expected the book, got {other:?}"),
        },
        other => panic!("expected a frame, got {other:?}"),
    }
}

#[tokio::test]
async fn a_dropped_socket_is_dialled_again_and_everything_resubscribed() {
    let (listener, url) = listen().await;
    let mut feed = Feed::start_at(url, Session::new(Network::Mainnet).with_timing(FAST));
    feed.subscribe(book("BTC")).await.unwrap();
    feed.subscribe(book("ETH")).await.unwrap();
    let mut first = accept(&listener).await;
    assert_eq!(text(&mut first).await, crate::live::subscribe(&book("BTC")));
    assert_eq!(text(&mut first).await, crate::live::subscribe(&book("ETH")));
    first.close(None).await.unwrap();
    drop(first);
    let mut second = accept(&listener).await;
    assert_eq!(text(&mut second).await, crate::live::subscribe(&book("BTC")));
    assert_eq!(text(&mut second).await, crate::live::subscribe(&book("ETH")));
    let mut saw_closed = false;
    while let Ok(Some(event)) = timeout(Duration::from_millis(200), feed.next()).await {
        saw_closed |= matches!(event, Event::Closed { .. });
    }
    assert!(saw_closed, "the drop is reported before the next open");
}

#[tokio::test]
async fn a_quiet_socket_is_pinged() {
    let (listener, url) = listen().await;
    let mut feed = Feed::start_at(url, Session::new(Network::Mainnet).with_timing(FAST));
    feed.subscribe(book("BTC")).await.unwrap();
    let mut server = accept(&listener).await;
    text(&mut server).await;
    assert_eq!(text(&mut server).await, crate::live::PING);
    server.send(Message::text(PONG)).await.unwrap();
    assert_eq!(event(&mut feed).await, Event::Opened);
    assert_eq!(event(&mut feed).await, Event::Frame(Box::new(Frame::Pong)));
}

#[tokio::test]
async fn a_socket_nothing_arrives_on_is_given_up_and_dialled_again() {
    let (listener, url) = listen().await;
    let _feed = Feed::start_at(url, Session::new(Network::Mainnet).with_timing(FAST));
    let silent = accept(&listener).await;
    let again = timeout(WAIT, listener.accept()).await;
    assert!(again.is_ok(), "a second connection after the silence");
    drop(silent);
}

#[tokio::test]
async fn a_post_goes_out_with_its_id_and_its_answer_comes_back() {
    let (listener, url) = listen().await;
    let mut feed = Feed::start_at(url, Session::new(Network::Mainnet));
    let mut server = accept(&listener).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    let request = Post::info(r##"{"type":"l2Book","coin":"#0"}"##).unwrap();
    let id = feed.post(request.clone()).await.unwrap();
    assert_eq!(text(&mut server).await, crate::live::post(id, &request));
    server.send(Message::text(NULL_ANSWER.replace(r#""id":11"#, &format!(r#""id":{id}"#)))).await.unwrap();
    match event(&mut feed).await {
        Event::Frame(frame) => match *frame {
            Frame::Post(reply) => assert_eq!(reply.id, id),
            other => panic!("expected the answer, got {other:?}"),
        },
        other => panic!("expected a frame, got {other:?}"),
    }
}

#[tokio::test]
async fn a_post_is_refused_while_the_socket_is_closed() {
    let (listener, url) = listen().await;
    drop(listener);
    let feed = Feed::start_at(url, Session::new(Network::Mainnet).with_timing(FAST));
    let request = Post::info(r#"{"type":"meta"}"#).unwrap();
    assert_eq!(feed.post(request).await, Err(FeedError::Refused(Refused::Closed)));
}

#[tokio::test]
async fn a_closed_feed_refuses_everything_after() {
    let (_listener, url) = listen().await;
    let feed = Feed::start_at(url, Session::new(Network::Mainnet));
    let commands = feed.commands.clone();
    feed.close();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(commands.is_closed(), "the driver stopped");
}
