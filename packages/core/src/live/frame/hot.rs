//! The busiest market channels (`l2Book`, `trades`, `bbo`) read straight from the text, in the one
//! compact layout the venue writes them: keys in its order, no whitespace, prices and sizes as
//! strings. Any other spelling is `None` and goes to the serde reader. Every frame read here is the
//! frame the serde reader gives; the tests check both on every recorded frame and every cut of one.

use crate::live::{Bbo, Frame};
use crate::markets::{Book, Level, Side, Trade};
use crate::utils::address::Address;

use super::scan::Scan;

/// `l2Book` sends at most twenty levels a side.
const BOOK_DEPTH: usize = 20;

/// One of the three channels, or `None`.
pub(super) fn read(text: &str) -> Option<Frame> {
    let mut scan = Scan::new(text);
    scan.expect(r#"{"channel":""#)?;
    let frame = if scan.eat(r#"l2Book","data":"#) {
        Frame::Book(book(&mut scan)?)
    } else if scan.eat(r#"trades","data":"#) {
        Frame::Trades(trades(&mut scan)?)
    } else if scan.eat(r#"bbo","data":"#) {
        Frame::Bbo(bbo(&mut scan)?)
    } else {
        return None;
    };
    scan.expect("}")?;
    scan.done().then_some(frame)
}

/// `{"coin":..,"time":..,"levels":[[..],[..]]}`, then `"spread"` when the book is bucketed and
/// `"fast"` on a fast subscription, which [`Book`] does not keep.
fn book(scan: &mut Scan<'_>) -> Option<Book> {
    scan.expect(r#"{"coin":"#)?;
    let coin = scan.string()?.to_owned();
    scan.expect(r#","time":"#)?;
    let time = scan.uint()?;
    scan.expect(r#","levels":["#)?;
    let bids = levels(scan)?;
    scan.expect(",")?;
    let asks = levels(scan)?;
    scan.expect("]")?;
    let spread = match scan.eat(r#","spread":"#) {
        true => Some(scan.dec()?),
        false => None,
    };
    if scan.eat(r#","fast":"#) && !scan.eat("true") {
        scan.expect("false")?;
    }
    scan.expect("}")?;
    Some(Book { coin, time, bids, asks, spread })
}

/// `[{"px":..,"sz":..,"n":..},..]`.
fn levels(scan: &mut Scan<'_>) -> Option<Vec<Level>> {
    scan.expect("[")?;
    let mut levels = Vec::with_capacity(BOOK_DEPTH);
    if scan.eat("]") {
        return Some(levels);
    }
    loop {
        levels.push(level(scan)?);
        if scan.eat("]") {
            return Some(levels);
        }
        scan.expect(",")?;
    }
}

fn level(scan: &mut Scan<'_>) -> Option<Level> {
    scan.expect(r#"{"px":"#)?;
    let px = scan.dec()?;
    scan.expect(r#","sz":"#)?;
    let sz = scan.dec()?;
    scan.expect(r#","n":"#)?;
    let n = u32::try_from(scan.uint()?).ok()?;
    scan.expect("}")?;
    Some(Level { px, sz, n })
}

/// `[{"coin","side","px","sz","time","hash","tid","users"},..]`.
fn trades(scan: &mut Scan<'_>) -> Option<Vec<Trade>> {
    scan.expect("[")?;
    let mut trades = Vec::new();
    if scan.eat("]") {
        return Some(trades);
    }
    loop {
        trades.push(trade(scan)?);
        if scan.eat("]") {
            return Some(trades);
        }
        scan.expect(",")?;
    }
}

fn trade(scan: &mut Scan<'_>) -> Option<Trade> {
    scan.expect(r#"{"coin":"#)?;
    let coin = scan.string()?.to_owned();
    scan.expect(r#","side":"#)?;
    let side = Side::parse(scan.string()?)?;
    scan.expect(r#","px":"#)?;
    let px = scan.dec()?;
    scan.expect(r#","sz":"#)?;
    let sz = scan.dec()?;
    scan.expect(r#","time":"#)?;
    let time = scan.uint()?;
    scan.expect(r#","hash":"#)?;
    let hash = scan.string()?.to_owned();
    scan.expect(r#","tid":"#)?;
    let tid = scan.uint()?;
    scan.expect(r#","users":["#)?;
    let buyer: Address = scan.parsed()?;
    scan.expect(",")?;
    let seller: Address = scan.parsed()?;
    scan.expect("]}")?;
    Some(Trade { coin, side, px, sz, time, hash, tid, buyer, seller })
}

/// `{"coin":..,"time":..,"bbo":[level or null, level or null]}`.
fn bbo(scan: &mut Scan<'_>) -> Option<Bbo> {
    scan.expect(r#"{"coin":"#)?;
    let coin = scan.string()?.to_owned();
    scan.expect(r#","time":"#)?;
    let time = scan.uint()?;
    scan.expect(r#","bbo":["#)?;
    let bid = side_of_bbo(scan)?;
    scan.expect(",")?;
    let ask = side_of_bbo(scan)?;
    scan.expect("]}")?;
    Some(Bbo { coin, time, bid, ask })
}

fn side_of_bbo(scan: &mut Scan<'_>) -> Option<Option<Level>> {
    match scan.peek()? {
        b'n' => scan.expect("null").map(|()| None),
        _ => level(scan).map(Some),
    }
}

#[cfg(test)]
#[path = "tests/hot.rs"]
mod tests;
