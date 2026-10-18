//! A market's tape and candles: prints kept newest first and once each, candles oldest first with
//! the one forming replaced, each up to its count.

use crate::client::Network;
use crate::live::Frame;
use crate::live::store::fixtures::{RECENT_TRADES, dec, frame, read};
use crate::live::store::market::fold_bar;
use crate::live::store::{BARS, Folded, Interest, Read, Store, TAPE};
use crate::markets::{Candle, Trade};

/// The recorded BTC print, its time and `tid` both set to `time`.
fn print(time: u64) -> Trade {
    let Frame::Trades(prints) = frame("trades") else { panic!("prints") };
    Trade { time, tid: time, ..prints[0].clone() }
}

/// The socket sends prints oldest first and `recentTrades` answers newest first; each is held
/// once, by its coin, `tid` and time, newest first, up to [`TAPE`].
#[test]
fn prints_are_kept_newest_first_once_each_and_up_to_the_tape() {
    let mut store = Store::new(Network::Mainnet);
    let _trades = store.register(Interest::Trades { coin: "BTC".to_owned() }).unwrap();
    let recent: Vec<Trade> = read(RECENT_TRADES);
    assert_eq!(store.read(1_000, Read::Trades(&recent)), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("trades")), Folded::Changed);
    assert_eq!(store.apply(3_000, &frame("trades")), Folded::Same, "the same prints again");
    assert_eq!(store.read(3_000, Read::Trades(&recent)), Folded::Same);
    let tape = store.trades("BTC").unwrap();
    let held: Vec<(u64, u64)> = tape.iter().map(|print| (print.time, print.tid)).collect();
    assert_eq!(
        held,
        [
            (1791480719939, 739564670599029),
            (1791480719435, 958487846715329),
            (1791480719236, 966065094835399),
            (1791480516231, 8986569314831),
            (1791480516231, 717978735161105),
        ]
    );
    assert_eq!((tape.heard, tape.changed), (3_000, 2_000));
    assert_eq!(store.apply(3_000, &frame("trades.spot")), Folded::Dropped, "no page wants @107's");

    let mut store = Store::new(Network::Mainnet);
    let _trades = store.register(Interest::Trades { coin: "BTC".to_owned() }).unwrap();
    let _ = store.apply(1, &Frame::Trades(vec![print(1), print(2)]));
    let _ = store.apply(2, &Frame::Trades(vec![print(3)]));
    let times: Vec<u64> = store.trades("BTC").unwrap().iter().map(|print| print.time).collect();
    assert_eq!(times, [3, 2, 1]);
    let _ = store.apply(3, &Frame::Trades((4..=250).map(print).collect()));
    let tape = store.trades("BTC").unwrap();
    assert_eq!(tape.len(), TAPE);
    assert_eq!((tape[0].time, tape[TAPE - 1].time), (250, 51), "the oldest go");
}

/// The recorded BTC minute candle, opening at `start` and closing at `close`.
fn bar(start: u64, close: &str) -> Candle {
    let Frame::Candle(candle) = frame("candle") else { panic!("a candle") };
    Candle { open_time: start, close: dec(close), ..candle }
}

/// The one forming is replaced, the next goes on the end, a late one lands on its own start,
/// and the oldest go past [`BARS`].
#[test]
fn a_candle_replaces_the_one_forming_and_the_next_goes_on_the_end() {
    let mut bars = vec![bar(0, "1"), bar(900, "2")];
    assert!(fold_bar(&mut bars, bar(900, "3")));
    assert_eq!((bars.len(), bars[1].close), (2, dec("3")), "the one forming is replaced");
    assert!(fold_bar(&mut bars, bar(1800, "4")));
    assert_eq!(bars.len(), 3, "the next is added");
    assert!(fold_bar(&mut bars, bar(0, "5")));
    assert_eq!((bars.len(), bars[0].close), (3, dec("5")), "a late one lands on its own start");
    assert!(!fold_bar(&mut bars, bar(0, "5")), "the same again changes nothing");
    assert!(!fold_bar(&mut bars, bar(450, "6")), "a late one with no start of its own is not kept");

    let mut bars = Vec::new();
    for at in 0..(BARS as u64 + 10) {
        let _ = fold_bar(&mut bars, bar(at * 900, "1"));
    }
    assert_eq!((bars.len(), bars[0].open_time), (BARS, 10 * 900), "the oldest go");
}
