//! The L4 books of one subscription, kept from a snapshot and the blocks that follow it.
//!
//! - **Subscribe first, buffer, then the snapshot.** The other way round, every block between
//!   the snapshot being taken and the socket attaching is lost, silently: the books keep orders
//!   that were cancelled and miss ones that were placed. Buffering first means that gap cannot
//!   exist; the blocks the snapshot already holds are skipped by height.
//! - **Blocks are contiguous.** The first block applied is the one right after the snapshot,
//!   and each after it is the next. A missed block (one never received, or a frame dropped
//!   because it did not read) leaves orders wrong in ways no later block repairs, so the books
//!   are thrown away and a new snapshot asked for ([`Step::Resync`]). A block from the past is a
//!   gap too, and so is a snapshot older than the first block buffered.
//!
//! The socket's owner feeds it: [`Assembler::block`] for every L4 frame of this subscription
//! (a socket holding several subscriptions routes by [`Block::coins`]), and
//! [`Assembler::snapshot`] once one comes back, a couple of seconds after subscribing and after
//! every resync.

use std::collections::{BTreeMap, VecDeque};

use crate::{book::Book, snapshot::Snapshot, socket::Block};

/// Blocks held while a snapshot is on its way, the newest kept: about five minutes of blocks.
/// If the snapshot never comes, this is the only thing that grows.
pub const MAX_BUFFER: usize = 4_096;

/// What one block or snapshot did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Held until a snapshot comes.
    Buffered,
    /// The books moved on.
    Applied,
    /// The snapshot already holds its changes.
    Skipped,
    /// Block `expected` is missing (`got` came instead): the books are gone and a new snapshot is
    /// needed. Blocks are buffered again meanwhile, starting with the one that showed the gap.
    Resync { expected: u64, got: u64 },
}

#[derive(Debug, Clone, Default)]
pub struct Assembler {
    books: BTreeMap<String, Book>,
    buffer: VecDeque<Block>,
    live: bool,
    /// The snapshot's height: blocks at or below it are already in the books.
    floor: u64,
    /// The last block applied, or the snapshot's height before any.
    height: u64,
    resyncs: u32,
}

impl Assembler {
    /// Waiting for its snapshot, buffering blocks.
    #[must_use]
    pub fn new() -> Assembler {
        Assembler::default()
    }

    /// Whether the books are whole and current.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.live
    }

    /// The block the books stand at; 0 before a snapshot.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }

    /// Times a gap threw the books away.
    #[must_use]
    pub const fn resyncs(&self) -> u32 {
        self.resyncs
    }

    /// One coin's book while live; none before a snapshot or after a gap.
    #[must_use]
    pub fn book(&self, coin: &str) -> Option<&Book> {
        self.books.get(coin)
    }

    pub fn books(&self) -> impl Iterator<Item = (&str, &Book)> {
        self.books.iter().map(|(coin, book)| (coin.as_str(), book))
    }

    /// A block from the socket: buffered while there is no snapshot, else applied if it is the
    /// next one.
    pub fn block(&mut self, block: Block) -> Step {
        if !self.live {
            if self.buffer.len() >= MAX_BUFFER {
                self.buffer.pop_front();
            }
            self.buffer.push_back(block);
            return Step::Buffered;
        }
        self.next(block)
    }

    /// The snapshot: the books replaced, then every buffered block above it applied in turn.
    pub fn snapshot(&mut self, snapshot: Snapshot) -> Step {
        self.books = snapshot.markets.iter().map(|market| (market.coin.clone(), Book::from_market(market))).collect();
        self.floor = snapshot.height;
        self.height = snapshot.height;
        self.live = true;
        while let Some(block) = self.buffer.pop_front() {
            if let step @ Step::Resync { .. } = self.next(block) {
                return step;
            }
        }
        Step::Applied
    }

    fn next(&mut self, block: Block) -> Step {
        if block.height <= self.floor {
            return Step::Skipped;
        }
        let expected = self.height.saturating_add(1);
        if block.height != expected {
            let got = block.height;
            self.resync(block);
            return Step::Resync { expected, got };
        }
        self.height = block.height;
        for diff in &block.diffs {
            self.books.entry(diff.coin().to_owned()).or_default().apply(diff);
        }
        Step::Applied
    }

    /// Throws the books away and buffers again from `block`, ahead of what was still buffered.
    fn resync(&mut self, block: Block) {
        self.books.clear();
        self.live = false;
        self.floor = 0;
        self.height = 0;
        self.resyncs = self.resyncs.saturating_add(1);
        self.buffer.push_front(block);
        while self.buffer.len() > MAX_BUFFER {
            self.buffer.pop_front();
        }
    }
}

#[cfg(test)]
#[path = "tests/assembler.rs"]
mod tests;
