//! Run as wasm in Node (`just web`), the feed on Node's own `WebSocket`, against a websocket
//! server on 127.0.0.1 started from the test (`server.js`). Its frames were recorded from
//! wss://api.hyperliquid.xyz/ws on 2026-10-08 (`fixtures/ws/mainnet.json`); the server only
//! puts back the id a post went out with.

use std::{
    future::poll_fn,
    pin::{Pin, pin},
    task::Poll,
};

use crate::client::Network;
use crate::live::{Frame, PING, Post, Refused, Session, Subscription, Timing};
use crate::utils::Json;
use js_sys::Promise;
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

use super::*;

#[wasm_bindgen(module = "/fixtures/web/server.js")]
extern "C" {
    type Server;
    #[wasm_bindgen(static_method_of = Server)]
    fn listen() -> Promise;
    #[wasm_bindgen(method)]
    fn url(this: &Server) -> String;
    #[wasm_bindgen(method)]
    fn accept(this: &Server, ms: u32) -> Promise;
    #[wasm_bindgen(method)]
    fn close(this: &Server);

    type Peer;
    #[wasm_bindgen(method)]
    fn text(this: &Peer, ms: u32) -> Promise;
    #[wasm_bindgen(method)]
    fn send(this: &Peer, text: &str);
    #[wasm_bindgen(method)]
    fn close(this: &Peer);
    #[wasm_bindgen(method)]
    fn ended(this: &Peer, ms: u32) -> Promise;

    type Deadline;
    #[wasm_bindgen(constructor)]
    fn new(ms: u32) -> Deadline;
    #[wasm_bindgen(method, getter)]
    fn promise(this: &Deadline) -> Promise;
    #[wasm_bindgen(method)]
    fn cancel(this: &Deadline);
}

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/mainnet.json"));

/// A frame from the mainnet recording, by its name there.
fn recorded(name: &str) -> String {
    let manifest = Json::parse(MAINNET).unwrap();
    let frames = manifest.get("frames").and_then(Json::as_object).unwrap();
    let (_, text) = frames.iter().find(|(each, _)| each == name).unwrap();
    text.as_str().unwrap().to_owned()
}

/// Quick enough for a test to see every rule fire.
const FAST: Timing =
    Timing { ping_after_ms: 150, silent_ms: 400, steady_ms: 10_000, first_retry_ms: 20, last_retry_ms: 40 };

/// As quick to dial again, but no ping in the time a test watches for nothing.
const QUIET: Timing = Timing { ping_after_ms: 10_000, silent_ms: 20_000, ..FAST };

const WAIT: u32 = 5_000;

fn book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}

async fn listen() -> Server {
    JsFuture::from(Server::listen()).await.unwrap().unchecked_into()
}

async fn accept(server: &Server) -> Peer {
    JsFuture::from(server.accept(WAIT)).await.unwrap().unchecked_into()
}

async fn text(peer: &Peer) -> String {
    JsFuture::from(peer.text(WAIT)).await.unwrap().as_string().unwrap()
}

/// `future`'s output, or `None` when `ms` passes first.
async fn within<T>(ms: u32, future: impl Future<Output = T>) -> Option<T> {
    let deadline = Deadline::new(ms);
    let mut future = pin!(future);
    let mut timeout = JsFuture::from(deadline.promise());
    let out = poll_fn(|cx| match future.as_mut().poll(cx) {
        Poll::Ready(out) => Poll::Ready(Some(out)),
        Poll::Pending => match Pin::new(&mut timeout).poll(cx) {
            Poll::Ready(_) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        },
    })
    .await;
    deadline.cancel();
    out
}

async fn event(feed: &mut Feed) -> Event {
    within(WAIT, feed.next()).await.expect("an event in time").expect("a running feed")
}

#[wasm_bindgen_test]
async fn it_subscribes_on_open_and_hands_back_typed_frames() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet));
    feed.subscribe(book("#14720")).await.unwrap();
    let peer = accept(&server).await;
    assert_eq!(text(&peer).await, crate::live::subscribe(&book("#14720")));
    peer.send(&recorded("l2Book.empty"));
    assert_eq!(event(&mut feed).await, Event::Opened);
    match event(&mut feed).await {
        Event::Frame(frame) => match *frame {
            Frame::Book(book) => assert_eq!((book.coin.as_str(), book.bids.len()), ("#14720", 0)),
            other => panic!("expected the book, got {other:?}"),
        },
        other => panic!("expected a frame, got {other:?}"),
    }
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_dropped_socket_is_dialled_again_and_everything_resubscribed() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet).with_timing(FAST));
    feed.subscribe(book("BTC")).await.unwrap();
    feed.subscribe(book("ETH")).await.unwrap();
    let first = accept(&server).await;
    assert_eq!(text(&first).await, crate::live::subscribe(&book("BTC")));
    assert_eq!(text(&first).await, crate::live::subscribe(&book("ETH")));
    first.close();
    let second = accept(&server).await;
    assert_eq!(text(&second).await, crate::live::subscribe(&book("BTC")));
    assert_eq!(text(&second).await, crate::live::subscribe(&book("ETH")));
    assert_eq!(event(&mut feed).await, Event::Opened);
    match event(&mut feed).await {
        Event::Closed { retry, .. } => {
            assert!((20..=25).contains(&retry.after_ms), "the first delay and a quarter: {}", retry.after_ms);
            assert!(retry.unanswered.is_empty());
        }
        other => panic!("the drop is reported before the next open, got {other:?}"),
    }
    assert_eq!(event(&mut feed).await, Event::Opened);
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_quiet_socket_is_pinged() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet).with_timing(FAST));
    feed.subscribe(book("BTC")).await.unwrap();
    let peer = accept(&server).await;
    text(&peer).await;
    assert_eq!(text(&peer).await, PING);
    peer.send(&recorded("pong"));
    assert_eq!(event(&mut feed).await, Event::Opened);
    assert_eq!(event(&mut feed).await, Event::Frame(Box::new(Frame::Pong)));
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_socket_nothing_arrives_on_is_given_up_and_dialled_again() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet).with_timing(FAST));
    let silent = accept(&server).await;
    assert_eq!(text(&silent).await, PING, "pinged first");
    let _again = accept(&server).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    match event(&mut feed).await {
        Event::Closed { reason, .. } => assert_eq!(reason, "nothing arrived for too long"),
        other => panic!("expected the silence, got {other:?}"),
    }
    assert_eq!(event(&mut feed).await, Event::Opened);
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_frame_that_does_not_read_leaves_the_socket_open() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet));
    let peer = accept(&server).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    peer.send("not json");
    assert!(matches!(event(&mut feed).await, Event::Unreadable(_)));
    peer.send(&recorded("pong"));
    assert_eq!(event(&mut feed).await, Event::Frame(Box::new(Frame::Pong)));
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_post_goes_out_with_its_id_and_its_answer_comes_back() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet));
    let peer = accept(&server).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    let request = Post::info(r##"{"type":"l2Book","coin":"#0"}"##).unwrap();
    let id = feed.post(request.clone()).await.unwrap();
    assert_eq!(text(&peer).await, crate::live::post(id, &request));
    peer.send(&recorded("post.info.null").replace(r#""id":11"#, &format!(r#""id":{id}"#)));
    match event(&mut feed).await {
        Event::Frame(frame) => match *frame {
            Frame::Post(reply) => assert_eq!(reply.id, id),
            other => panic!("expected the answer, got {other:?}"),
        },
        other => panic!("expected a frame, got {other:?}"),
    }
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn an_unanswered_post_is_reported_on_close_and_never_sent_again() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet).with_timing(QUIET));
    feed.subscribe(book("BTC")).await.unwrap();
    let first = accept(&server).await;
    text(&first).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    let id = feed.post(Post::info(r#"{"type":"meta"}"#).unwrap()).await.unwrap();
    text(&first).await;
    first.close();
    match event(&mut feed).await {
        Event::Closed { retry, .. } => assert_eq!(retry.unanswered, vec![id]),
        other => panic!("expected the close, got {other:?}"),
    }
    let second = accept(&server).await;
    assert_eq!(text(&second).await, crate::live::subscribe(&book("BTC")));
    assert!(JsFuture::from(second.text(300)).await.is_err(), "the post is not sent again");
    feed.close();
    server.close();
}

#[wasm_bindgen_test]
async fn a_socket_that_cannot_open_refuses_posts_and_is_dialled_again_later_each_time() {
    let server = listen().await;
    let url = server.url();
    server.close();
    let mut feed = Feed::start_at(url, Session::new(Network::Mainnet).with_timing(FAST));
    let request = Post::info(r#"{"type":"meta"}"#).unwrap();
    assert_eq!(feed.post(request).await, Err(FeedError::Refused(Refused::Closed)));
    let mut delays = Vec::new();
    for _ in 0..2 {
        match event(&mut feed).await {
            Event::Closed { reason, retry } => {
                assert!(!reason.is_empty());
                assert!(retry.unanswered.is_empty());
                delays.push(retry.after_ms);
            }
            other => panic!("expected the failed dial, got {other:?}"),
        }
    }
    assert!((20..=25).contains(&delays[0]), "the first delay and a quarter: {delays:?}");
    assert!((40..=50).contains(&delays[1]), "doubled: {delays:?}");
    feed.close();
}

#[wasm_bindgen_test]
async fn a_closed_feed_closes_its_socket_and_dials_no_more() {
    let server = listen().await;
    let mut feed = Feed::start_at(server.url(), Session::new(Network::Mainnet).with_timing(QUIET));
    let peer = accept(&server).await;
    assert_eq!(event(&mut feed).await, Event::Opened);
    feed.close();
    JsFuture::from(peer.ended(WAIT)).await.expect("the socket ends");
    assert!(JsFuture::from(server.accept(300)).await.is_err(), "no dial after the close");
    server.close();
}
