//! Two builders' files for 2026-09-26 from Hyperliquid's bucket, stored as sent: LZ4 frames
//! around a CSV. The sums were taken from the same files opened by the `lz4` tool and summed in
//! Python's `decimal` (see `fixtures/README.md`).

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{Http, Method, Request, TransportError};

use super::*;

const WIDE: &[u8] = include_bytes!("fixtures/builder_fills_0c8d970c_20260926.csv.lz4");
const SMALL: &[u8] = include_bytes!("fixtures/builder_fills_8d62d300_20260926.csv.lz4");
const WIDE_BUILDER: &str = "0x0c8d970c462726e014ad36f6c5a63e99db48a8e7";
const SMALL_BUILDER: &str = "0x8d62d3000ef0639d1fc9667d06be7bb98d9993f5";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn day(bytes: &[u8], builder: &str) -> BuilderDay {
    let csv = unframe(bytes).unwrap();
    read_day(address(builder), "20260926", &String::from_utf8(csv).unwrap()).unwrap()
}

#[test]
fn a_frame_opens_to_the_csv_with_its_header_first() {
    let text = String::from_utf8(unframe(SMALL).unwrap()).unwrap();
    assert!(text.starts_with("time,user,coin,side,px,sz,crossed,"));
    assert_eq!(text.lines().count(), 22, "a header and 21 fills");
}

#[test]
fn a_day_splits_native_perps_hip3_spot_and_outcomes() {
    let day = day(WIDE, WIDE_BUILDER);
    assert_eq!(day.users, 12);
    assert_eq!((day.native.volume, day.native.fills), (dec("5086.28499"), 110));
    assert_eq!((day.hip3.volume, day.hip3.fills), (dec("19253.036596"), 66));
    assert_eq!((day.spot.volume, day.spot.fills), (dec("19169.84488842"), 1589));
    assert_eq!((day.outcomes.volume, day.outcomes.fills), (dec("6455.9099"), 107));
    assert_eq!(day.perps().fills, 176);
    assert_eq!(day.builder, address(WIDE_BUILDER));
}

#[test]
fn hip3_volume_is_kept_per_dex_the_busiest_first() {
    let day = day(WIDE, WIDE_BUILDER);
    let dexes: Vec<(&str, Dec, u64)> =
        day.dexes.iter().map(|(dex, tally)| (dex.as_str(), tally.volume, tally.fills)).collect();
    assert_eq!(dexes, [("xyz", dec("16652.742296"), 51), ("io", dec("2600.2943"), 15)]);
    assert!(day.coins.iter().all(|(coin, _)| !coin.starts_with('@') && !coin.starts_with('#')), "perps only");
    let total = day.coins.iter().fold(Dec::ZERO, |sum, (_, tally)| sum.checked_add(tally.volume).unwrap());
    assert_eq!(total, day.perps().volume, "the coins add up to the perps");
}

#[test]
fn builder_fees_are_summed_as_paid() {
    let day = day(SMALL, SMALL_BUILDER);
    assert_eq!(day.hip3.fees, dec("1.727559"));
    assert_eq!(day.native.fees, dec("0.132636"));
    assert_eq!(day.perps().fees, dec("1.860195"));
    assert_eq!((day.native.volume, day.hip3.volume), (dec("331.596052"), dec("4318.91525")));
    assert_eq!(day.users, 6);
}

#[test]
fn a_coin_is_told_apart_by_how_the_venue_writes_it() {
    assert_eq!(kind_of("BTC"), Kind::Native);
    assert_eq!(kind_of("xyz:TSLA"), Kind::Hip3("xyz"));
    assert_eq!(kind_of("@107"), Kind::Spot);
    assert_eq!(kind_of("PURR/USDC"), Kind::Spot);
    assert_eq!(kind_of("#1230"), Kind::Outcome);
}

#[test]
fn a_file_that_is_not_a_whole_lz4_frame_is_refused() {
    assert!(unframe(b"<?xml version=\"1.0\"?><Error>AccessDenied</Error>").is_err());
    for end in [1, 4, 6, 7, 11, SMALL.len() / 2, SMALL.len() - 1] {
        assert!(unframe(&SMALL[..end]).is_err(), "cut at {end}");
    }
}

#[test]
fn a_csv_without_the_columns_it_sums_is_refused() {
    assert!(read_day(address(SMALL_BUILDER), "20260926", "time,user,coin\n").is_err());
    assert!(read_day(address(SMALL_BUILDER), "20260926", "").is_err());
}

#[test]
fn the_file_is_asked_for_by_builder_and_day_without_any_key() {
    let request = request(address(SMALL_BUILDER), "20260926").unwrap();
    assert_eq!(request.method, Method::Get);
    assert_eq!(
        request.url,
        "https://stats-data.hyperliquid.xyz/Mainnet/builder_fills/0x8d62d3000ef0639d1fc9667d06be7bb98d9993f5/20260926.csv.lz4"
    );
    assert!(request.headers.is_empty(), "nothing of the Hydromancer key goes to Hyperliquid's bucket");
    for bad in ["2026-09-26", "2026092", "202609261", "2026092x", "../../x"] {
        assert!(super::request(address(SMALL_BUILDER), bad).is_err(), "{bad}");
    }
}

struct Wire {
    answer: RefCell<Option<Response>>,
}

impl Http for Wire {
    fn send(&self, _: Request) -> impl Future<Output = Result<Response, TransportError>> {
        let answer =
            self.answer.borrow_mut().take().ok_or_else(|| TransportError::Unreachable("asked twice".to_owned()));
        ready(answer)
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

fn wire(status: u16, body: &[u8]) -> Wire {
    Wire { answer: RefCell::new(Some(Response { status, headers: Vec::new(), body: body.to_vec() })) }
}

#[test]
fn a_day_with_no_file_is_none_and_a_file_is_its_day() {
    let builder = address(SMALL_BUILDER);
    let denied = b"<?xml version=\"1.0\"?><Error><Code>AccessDenied</Code></Error>";
    assert_eq!(block_on(builder_day(&wire(403, denied), builder, "20260926")), Ok(None));
    assert_eq!(block_on(builder_day(&wire(404, b""), builder, "20260926")), Ok(None));
    let read = block_on(builder_day(&wire(200, SMALL), builder, "20260926")).unwrap().unwrap();
    assert_eq!(read.perps().fees, dec("1.860195"));
    assert!(block_on(builder_day(&wire(500, b"oops"), builder, "20260926")).is_err());
    assert!(block_on(builder_day(&wire(200, b"not lz4"), builder, "20260926")).is_err());
}
