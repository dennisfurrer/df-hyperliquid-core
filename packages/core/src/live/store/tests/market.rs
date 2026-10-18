use crate::client::Network;
use crate::live::Frame;
use crate::live::store::fixtures::{RECENT_TRADES, dec, frame, read};
use crate::live::store::market::fold_bar;
use crate::live::store::{BARS, Folded, Handle, Interest, Read, Store};
use crate::markets::{Candle, Trade};

fn store() -> Store {
    Store::new(Network::Mainnet)
}

fn wanting(store: &mut Store, interests: impl IntoIterator<Item = Interest>) -> Vec<Handle> {
    interests.into_iter().map(|interest| store.register(interest).unwrap()).collect()
}

fn coin(name: &str) -> String {
    name.to_owned()
}

/// Prints are kept newest first. The socket sends them oldest first; `recentTrades` answers
/// newest first; each print is held once, by its coin, `tid` and time.
#[test]
fn prints_are_kept_newest_first_and_once() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
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
}

/// The recorded BTC print, its time and `tid` both set to `time`.
fn print(time: u64) -> Trade {
    let Frame::Trades(prints) = frame("trades") else { panic!("prints") };
    Trade { time, tid: time, ..prints[0].clone() }
}

#[test]
fn prints_come_oldest_first_and_are_kept_newest_first() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
    let _ = store.apply(1, &Frame::Trades(vec![print(1), print(2)]));
    let _ = store.apply(2, &Frame::Trades(vec![print(3)]));
    let times: Vec<u64> = store.trades("BTC").unwrap().iter().map(|print| print.time).collect();
    assert_eq!(times, [3, 2, 1]);
}

#[test]
fn the_tape_stops_at_its_count() {
    let mut store = store();
    let _trades = wanting(&mut store, [Interest::Trades { coin: coin("BTC") }]);
    let prints: Vec<Trade> = (1..=250).map(print).collect();
    let _ = store.apply(1, &Frame::Trades(prints));
    let tape = store.trades("BTC").unwrap();
    assert_eq!(tape.len(), crate::live::store::TAPE);
    assert_eq!((tape[0].time, tape[crate::live::store::TAPE - 1].time), (250, 51), "the oldest go");
}

/// The recorded BTC minute candle, opening at `start` and closing at `close`.
fn bar(start: u64, close: &str) -> Candle {
    let Frame::Candle(candle) = frame("candle") else { panic!("a candle") };
    Candle { open_time: start, close: dec(close), ..candle }
}

#[test]
fn a_candle_replaces_the_one_forming_and_the_next_goes_on_the_end() {
    let mut bars = vec![bar(0, "1"), bar(900, "2")];
    assert!(fold_bar(&mut bars, bar(900, "3")));
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[1].close, dec("3"), "the one forming is replaced");
    assert!(fold_bar(&mut bars, bar(1800, "4")));
    assert_eq!(bars.len(), 3, "the next is added");
    assert!(fold_bar(&mut bars, bar(0, "5")));
    assert_eq!(bars[0].close, dec("5"), "a late one lands on its own start");
    assert_eq!(bars.len(), 3);
    assert!(!fold_bar(&mut bars, bar(0, "5")), "the same again changes nothing");
}

#[test]
fn the_candles_stop_at_their_count() {
    let mut bars = Vec::new();
    for at in 0..(BARS as u64 + 10) {
        let _ = fold_bar(&mut bars, bar(at * 900, "1"));
    }
    assert_eq!(bars.len(), BARS);
    assert_eq!(bars[0].open_time, 10 * 900, "the oldest go");
}
