//! Prices and the block height against HyperEVM mainnet's answers and viem's reading of them.

use crate::client::evm::precompiles::support::{s, sent, viem};

use super::*;

#[test]
fn prices_and_blocks_read_as_the_chain_wrote_them() {
    let mark = sent(&mark_px(0), "markPx");
    assert_eq!(mark.to_string(), s(&viem("markPx")));
    assert_eq!(sent(&oracle_px(0), "oraclePx").to_string(), s(&viem("oraclePx")));
    let spot = sent(&spot_px(107), "spotPx");
    assert_eq!(spot.to_string(), s(&viem("spotPx")));
    assert_eq!(sent(&l1_block_number(), "l1BlockNumber").to_string(), s(&viem("l1BlockNumber")));
    assert!(l1_block_number().data.is_empty(), "the one precompile with no arguments");
    let bbo = sent(&bbo(0), "bbo");
    assert_eq!([bbo.bid.to_string(), bbo.ask.to_string()], [s(&viem("bbo")["bid"]), s(&viem("bbo")["ask"])]);
    // BTC has five size decimals: a perp price is in 10^(6 - 5).
    assert_eq!(perp_price(mark, 5), Dec::new(i128::from(mark), 1));
    assert_eq!(perp_price(807_518, 5), Dec::parse("80751.8").ok());
    // HYPE, the base of pair 107, has two: a spot price is in 10^(8 - 2).
    assert_eq!(spot_price(83_222_000, 2), Dec::parse("83.222").ok());
    assert_eq!(perp_price(1, 7), None, "no perp has more than six size decimals");
    assert_eq!(spot_price(1, 9), None);
}
