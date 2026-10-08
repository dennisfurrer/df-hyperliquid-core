//! The assembler against Hydromancer's own Python SDK: after the snapshot and after each block,
//! every level of every book is what the SDK's `L4OrderbookClient` holds (`l4_books.json`), and
//! the SDK refuses the same gap. Queue order inside a level, which the SDK does not keep, follows
//! the documented `insertBefore` rule. See `fixtures/README.md`.

use std::collections::BTreeMap;

use hl_core::Dec;
use serde_json::Value as Read;

use super::*;
use crate::{
    book::Level,
    snapshot::{Side, decode},
    socket::{Message, read},
};

const FRAME: &[u8] = include_bytes!("fixtures/l4_snapshot.bin");
const BOOKS: &str = include_str!("fixtures/l4_books.json");
const B304: &str = include_str!("fixtures/l4_update_782007304.json");
const B305: &str = include_str!("fixtures/l4_update_782007305.json");
const B307: &str = include_str!("fixtures/l4_update_782007307.json");

fn snapshot() -> Snapshot {
    decode(FRAME).unwrap()
}

fn block(text: &str) -> Block {
    match read(text) {
        Ok(Message::Block(block)) => block,
        other => panic!("{other:?}"),
    }
}

fn empty(height: u64) -> Block {
    Block { height, time: 0, coins: None, diffs: Vec::new() }
}

/// What the SDK held at `height`: coin to `(bids, asks)`.
fn sdk(height: &str) -> BTreeMap<String, (Vec<Level>, Vec<Level>)> {
    let books: Read = serde_json::from_str(BOOKS).unwrap();
    let side = |levels: &Read| -> Vec<Level> {
        levels
            .as_array()
            .unwrap()
            .iter()
            .map(|level| Level {
                px: Dec::parse(level[0].as_str().unwrap()).unwrap(),
                sz: Dec::parse(level[1].as_str().unwrap()).unwrap(),
                orders: usize::try_from(level[2].as_u64().unwrap()).unwrap(),
            })
            .collect()
    };
    books["books"][height]
        .as_object()
        .unwrap()
        .iter()
        .map(|(coin, book)| (coin.clone(), (side(&book["bids"]), side(&book["asks"]))))
        .collect()
}

fn ours(assembler: &Assembler) -> BTreeMap<String, (Vec<Level>, Vec<Level>)> {
    assembler.books().map(|(coin, book)| (coin.to_owned(), (book.levels(Side::Buy), book.levels(Side::Sell)))).collect()
}

fn queue(assembler: &Assembler, px: &str) -> Vec<u64> {
    let book = assembler.book("ETH").unwrap();
    book.queue(Side::Buy, Dec::parse(px).unwrap()).map(|order| order.oid).collect()
}

#[test]
fn each_block_leaves_the_books_the_sdk_holds() {
    let mut assembler = Assembler::new();
    assert!(!assembler.is_live());
    assert_eq!(assembler.snapshot(snapshot()), Step::Applied);
    assert!(assembler.is_live());
    assert_eq!(assembler.height(), 782_007_303);
    assert_eq!(ours(&assembler), sdk("782007303"));

    assert_eq!(assembler.block(block(B304)), Step::Applied);
    assert_eq!(ours(&assembler), sdk("782007304"));
    assert_eq!(queue(&assembler, "3245.5"), [217_148_811_877, 217_148_811_876], "the ALO order went in front");

    assert_eq!(assembler.block(block(B305)), Step::Applied);
    assert_eq!(ours(&assembler), sdk("782007305"));
    assert!(assembler.book("ETH").unwrap().get(217_148_811_920).is_none(), "the crossing ask filled whole");
    assert_eq!(assembler.height(), 782_007_305);
}

#[test]
fn blocks_that_came_before_the_snapshot_are_buffered_then_drained_above_it() {
    let mut assembler = Assembler::new();
    assert_eq!(assembler.block(block(B304)), Step::Buffered);
    assert_eq!(assembler.block(block(B305)), Step::Buffered);
    assert!(assembler.books().next().is_none(), "no book before a snapshot");
    assert_eq!(assembler.snapshot(snapshot()), Step::Applied);
    assert_eq!(ours(&assembler), sdk("782007305"));
    assert_eq!(assembler.height(), 782_007_305);
}

#[test]
fn a_block_the_snapshot_already_holds_is_skipped() {
    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    assert_eq!(assembler.block(empty(782_007_302)), Step::Skipped);
    assert_eq!(assembler.block(empty(782_007_303)), Step::Skipped);
    assert_eq!(ours(&assembler), sdk("782007303"));
    assert!(assembler.is_live());
}

#[test]
fn a_height_gap_throws_the_books_away_and_asks_for_a_new_snapshot() {
    let books: Read = serde_json::from_str(BOOKS).unwrap();
    assert_eq!(books["gap"], "Height gap detected: 782007305 -> 782007307", "the SDK refuses it too");

    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    assembler.block(block(B304));
    assembler.block(block(B305));
    assert_eq!(assembler.block(block(B307)), Step::Resync { expected: 782_007_306, got: 782_007_307 });
    assert!(!assembler.is_live());
    assert!(assembler.books().next().is_none(), "a book with a block missing is wrong in ways no block repairs");
    assert_eq!(assembler.resyncs(), 1);
}

#[test]
fn a_frame_that_does_not_read_is_dropped_and_its_gap_resyncs() {
    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    assembler.block(block(B304));
    assert!(read(&B305.replace("\"3245.00\"", "\"3245.0x\"")).is_err(), "dropped whole, not in part");
    let next = B307.replace("782007307", "782007306");
    assert_eq!(assembler.block(block(&next)), Step::Resync { expected: 782_007_305, got: 782_007_306 });
}

#[test]
fn a_block_from_the_past_is_a_gap_too() {
    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    assembler.block(block(B304));
    assembler.block(block(B305));
    assert_eq!(assembler.block(block(B305)), Step::Resync { expected: 782_007_306, got: 782_007_305 });
}

#[test]
fn a_snapshot_older_than_the_first_block_buffered_is_a_gap() {
    let mut assembler = Assembler::new();
    assert_eq!(assembler.block(block(B305)), Step::Buffered);
    assert_eq!(assembler.snapshot(snapshot()), Step::Resync { expected: 782_007_304, got: 782_007_305 });
    assert!(!assembler.is_live());
    assert_eq!(assembler.block(empty(782_007_306)), Step::Buffered, "the next snapshot drains what follows");
}

#[test]
fn a_gap_inside_the_buffer_shows_when_the_snapshot_drains_it() {
    let mut assembler = Assembler::new();
    assembler.block(block(B304));
    assembler.block(block(B307));
    assert_eq!(assembler.snapshot(snapshot()), Step::Resync { expected: 782_007_305, got: 782_007_307 });
}

#[test]
fn after_a_resync_the_next_snapshot_starts_the_books_again() {
    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    assembler.block(block(B305));
    assert!(!assembler.is_live());
    let mut newer = snapshot();
    newer.height = 782_007_304;
    assert_eq!(assembler.snapshot(newer), Step::Applied, "the block that showed the gap was kept, and drains");
    assert!(assembler.is_live());
    assert_eq!(assembler.height(), 782_007_305);
}

#[test]
fn the_buffer_keeps_the_newest_blocks_when_the_snapshot_is_slow() {
    let mut assembler = Assembler::new();
    let first = 782_007_304;
    let past = u64::try_from(MAX_BUFFER).unwrap() + 10;
    for height in first..first + past {
        assembler.block(empty(height));
    }
    assert_eq!(
        assembler.snapshot(snapshot()),
        Step::Resync { expected: first, got: first + 10 },
        "the oldest ten were let go, so a snapshot below them has a gap"
    );
    let mut assembler = Assembler::new();
    for height in first..first + past {
        assembler.block(empty(height));
    }
    let mut late = snapshot();
    late.height = first + 9;
    assert_eq!(assembler.snapshot(late), Step::Applied);
    assert_eq!(assembler.height(), first + past - 1);
}

#[test]
fn a_coin_the_snapshot_did_not_hold_starts_from_an_empty_book() {
    let mut assembler = Assembler::new();
    assembler.snapshot(snapshot());
    let text = B304.replace("\"ETH\"", "\"SOL\"");
    assert_eq!(assembler.block(block(&text)), Step::Applied);
    let sol = assembler.book("SOL").unwrap();
    assert_eq!(sol.levels(Side::Buy).len(), 1);
    assert_eq!(sol.len(), 2);
}
