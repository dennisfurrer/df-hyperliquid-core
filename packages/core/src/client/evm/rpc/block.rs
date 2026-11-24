//! [`BlockTag`]: which block a read is at, written as JSON-RPC takes it.

use std::fmt;

/// Which block a read is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockTag {
    #[default]
    /// The newest block (`latest`).
    Latest,
    /// Counting what waits in the pool: the next nonce to use.
    Pending,
    /// The first block (`earliest`).
    Earliest,
    /// The newest block the node treats as safe from reorganisation (`safe`).
    Safe,
    /// The newest finalized block (`finalized`).
    Finalized,
    /// The block with this number, written as a `0x` hex quantity.
    Number(u64),
}

impl fmt::Display for BlockTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockTag::Latest => f.write_str("latest"),
            BlockTag::Pending => f.write_str("pending"),
            BlockTag::Earliest => f.write_str("earliest"),
            BlockTag::Safe => f.write_str("safe"),
            BlockTag::Finalized => f.write_str("finalized"),
            BlockTag::Number(number) => write!(f, "{number:#x}"),
        }
    }
}
