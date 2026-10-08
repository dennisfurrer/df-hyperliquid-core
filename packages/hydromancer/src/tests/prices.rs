//! `perpPriceHistoryByTime` in its batch form. `prices_live.json` is a live mainnet answer for a
//! native coin and a HIP-3 coin (hip4-tui, 2026-09-19); the request and the absent fields follow
//! Hydromancer's page for the route. Every expected figure is read off the answer by eye.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{Http, Json, Request, Response, TransportError};

use super::*;
use crate::client::{Error, Hydromancer, Key};

const LIVE: &[u8] = include_bytes!("fixtures/prices_live.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn live(coin: &str) -> Vec<PricePoint> {
    read_history(LIVE).unwrap().remove(coin).unwrap()
}

fn at(time: u64) -> PricePoint {
    PricePoint {
        time,
        block: 0,
        dex: "hyperliquid".to_owned(),
        coin: "ZEC".to_owned(),
        mark: dec("1513.8"),
        oracle: None,
        external: None,
        class: None,
    }
}

fn times(points: &[&PricePoint]) -> Vec<u64> {
    points.iter().map(|point| point.time).collect()
}

#[test]
fn one_answer_carries_a_native_coin_and_a_hip3_coin_together() {
    let history = read_history(LIVE).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!((history["ZEC"].len(), history["xyz:GOLD"].len()), (4, 2));
    assert_eq!((history["ZEC"][0].dex.as_str(), history["xyz:GOLD"][0].dex.as_str()), ("hyperliquid", "xyz"));
    assert_eq!(history["ZEC"][3].class, None, "`updateClass` is absent on the native dex, not empty");
    assert_eq!(history["xyz:GOLD"][1].class.as_deref(), Some("Normal"));
}

#[test]
fn every_figure_on_a_point_is_exact() {
    let newest = live("ZEC").pop().unwrap();
    assert_eq!((newest.time, newest.block), (1_789_835_708_778, 1_153_543_947));
    assert_eq!(newest.coin, "ZEC");
    assert_eq!(newest.mark, dec("1513.80"));
    assert_eq!(newest.oracle, Some(dec("1513.60")));
    assert_eq!(newest.external, Some(dec("1512.905")));
}

#[test]
fn the_points_stay_in_the_order_the_provider_sent_them() {
    let points = live("ZEC");
    for pair in points.windows(2) {
        let gap = pair[1].time - pair[0].time;
        assert!((2_000..4_000).contains(&gap), "a round about every {TYPICAL_GAP_MS} ms: {gap}");
    }
}

#[test]
fn the_point_at_or_before_an_instant_is_the_last_that_had_happened() {
    let points = live("ZEC");
    let chosen = point_at_or_before(&points, 1_789_835_707_000).unwrap();
    assert_eq!((chosen.time, chosen.mark), (1_789_835_705_686, dec("1513.2")));
    assert_eq!(point_at_or_before(&points, 1_789_835_705_686).unwrap().time, 1_789_835_705_686, "on the instant");
    assert_eq!(point_at_or_before(&points, u64::MAX).unwrap().time, 1_789_835_708_778);
    assert!(point_at_or_before(&points, 1_789_835_699_601).is_none(), "a later price is not the price at this instant");
    let shuffled = vec![at(3_000), at(1_000), at(2_000)];
    assert_eq!(point_at_or_before(&shuffled, 2_500).unwrap().time, 2_000, "whatever order they came in");
}

#[test]
fn the_nearest_points_are_the_closest_and_come_back_in_time_order() {
    let points = live("ZEC");
    assert_eq!(
        times(&nearest(&points, 1_789_835_706_000, 3)),
        [1_789_835_702_694, 1_789_835_705_686, 1_789_835_708_778]
    );
    assert_eq!(times(&nearest(&points, 1_789_835_708_778, 2)), [1_789_835_705_686, 1_789_835_708_778]);
    assert_eq!(nearest(&points, 1_789_835_706_000, 99).len(), 4);
    assert!(nearest(&points, 1_789_835_706_000, 0).is_empty());
    assert!(nearest(&[], 1_789_835_706_000, 3).is_empty());
    let tie = vec![at(1_000), at(2_000)];
    assert_eq!(times(&nearest(&tie, 1_500, 1)), [1_000], "a tie goes to the earlier point");
}

#[test]
fn a_point_that_does_not_read_is_dropped_without_its_neighbours() {
    let text = br#"{"ZEC":[
        {"blockNumber":1,"coin":"ZEC","dex":"hyperliquid","markPx":"1512.10","oraclePx":"1512.00","time":1000},
        {"blockNumber":2,"coin":"ZEC","dex":"hyperliquid","oraclePx":"1512.40","time":2000},
        {"blockNumber":3,"coin":"ZEC","dex":"hyperliquid","markPx":"","oraclePx":"1513.10","time":3000},
        {"markPx":"1512.60"},
        {"markPx":"1513.20","time":"not a time"},
        "not a point",
        {"blockNumber":4,"coin":"ZEC","dex":"hyperliquid","markPx":"1513.80","oraclePx":"1513.60","time":4000}
    ]}"#;
    let points = read_history(text).unwrap().remove("ZEC").unwrap();
    assert_eq!(points.iter().map(|point| point.time).collect::<Vec<_>>(), [1_000, 4_000]);
}

#[test]
fn a_point_naming_no_coin_takes_the_name_it_was_answered_under() {
    let points = read_history(br#"{"xyz:GOLD":[{"markPx":"4377.10","time":4000}]}"#).unwrap().remove("xyz:GOLD");
    let point = &points.unwrap()[0];
    assert_eq!(point.coin, "xyz:GOLD");
    assert_eq!((point.oracle, point.external), (None, None), "no reference, not a guess");
}

#[test]
fn ten_places_are_kept_rather_than_cut() {
    let points = read_history(br#"{"ZEC":[{"markPx":"1513.8012345678","oraclePx":"1513.6000000001","time":4000}]}"#)
        .unwrap()
        .remove("ZEC")
        .unwrap();
    assert_eq!(points[0].mark, Dec::new(15_138_012_345_678, 10).unwrap());
    assert_eq!(points[0].oracle, Some(Dec::new(15_136_000_000_001, 10).unwrap()));
}

#[test]
fn an_answer_that_is_not_keyed_by_coin_is_refused() {
    assert!(read_history(b"[]").is_err());
    assert!(read_history(b"not json").is_err());
}

#[test]
fn the_request_names_the_coins_and_the_bounds_and_nothing_else() {
    let call = history(&["xyz:GOLD", "ZEC"], 1_789_835_688_778, 1_789_835_708_778);
    assert_eq!(call.url(), "https://api.hydromancer.xyz/info");
    assert_eq!(
        call.body(),
        r#"{"type":"perpPriceHistoryByTime","coins":["xyz:GOLD","ZEC"],"startTime":1789835688778,"endTime":1789835708778}"#
    );
    assert_eq!(call.name(), "perpPriceHistoryByTime");
}

#[test]
fn a_failed_request_marks_every_coin_and_a_skipped_coin_is_named() {
    let coins = ["ZEC", "xyz:GOLD"];
    let failed = Error::Status { name: "perpPriceHistoryByTime".to_owned(), status: 503, said: String::new() };
    let marked = window(&coins, Err(failed));
    assert!(marked.history.is_empty());
    assert_eq!(marked.errors.iter().map(|(coin, _)| coin.as_str()).collect::<Vec<_>>(), coins);
    assert!(marked.errors.iter().all(|(_, said)| said.contains("503")));

    let mut history = read_history(LIVE).unwrap();
    history.insert("PURR".to_owned(), Vec::new());
    let named = window(&["ZEC", "PURR", "xyz:SILVER"], Ok(history));
    assert_eq!(named.errors.iter().map(|(coin, _)| coin.as_str()).collect::<Vec<_>>(), ["PURR", "xyz:SILVER"]);
    assert!(!named.history.contains_key("PURR"), "an empty list would read as a coin that did not move");
    assert_eq!(named.history["ZEC"].len(), 4);
}

/// Answers each request with the next recorded body, and keeps what was asked.
struct Wire {
    bodies: RefCell<Vec<String>>,
    sent: RefCell<Vec<Request>>,
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        let body = self.bodies.borrow_mut().pop().unwrap_or_else(|| "{}".to_owned());
        ready(Ok(Response { status: 200, headers: Vec::new(), body: body.into_bytes() }))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn asking_for_no_coins_asks_the_provider_nothing() {
    let wire = Wire { bodies: RefCell::default(), sent: RefCell::default() };
    let client = Hydromancer::mainnet(wire, Key::new("k").unwrap());
    assert!(block_on(client.price_history(&[], 0, 1)).unwrap().is_empty());
    assert!(client.http().sent.borrow().is_empty());
}

#[test]
fn more_coins_than_one_request_takes_go_out_in_batches_of_twenty() {
    let names: Vec<String> = (0..45).map(|n| format!("C{n}")).collect();
    let coins: Vec<&str> = names.iter().map(String::as_str).collect();
    let wire = Wire {
        bodies: RefCell::new(vec![r#"{"C44":[{"markPx":"1","time":5}]}"#.to_owned()]),
        sent: RefCell::default(),
    };
    let client = Hydromancer::mainnet(wire, Key::new("k").unwrap());
    let history = block_on(client.price_history(&coins, 0, 10)).unwrap();
    let sent = client.http().sent.borrow();
    assert_eq!(sent.len(), 3);
    let asked = |at: usize| Json::parse(&String::from_utf8_lossy(&sent[at].body)).unwrap();
    let count = |at: usize| asked(at).get("coins").and_then(Json::as_array).map_or(0, <[Json]>::len);
    assert_eq!((count(0), count(1), count(2)), (MAX_COINS, MAX_COINS, 5));
    assert_eq!(history["C44"][0].mark, Dec::ONE);
}
