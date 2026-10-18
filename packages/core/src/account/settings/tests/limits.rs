//! What an address may still send and the venue's formulas for what it earns, held against live
//! `userRateLimit` answers and the docs. The read runs in the mocked journeys.

use super::*;
use crate::account::fixtures::{RATE_LIMIT_BUILDER, RATE_LIMIT_BUILDER_TESTNET, RATE_LIMIT_UNIFIED, dec, read};

/// Recorded answers: ten decimals of volume, and a very large volume.
const TEN_DECIMALS: &str =
    r#"{"cumVlm":"1498565.3600000001","nRequestsUsed":541,"nRequestsCap":1508565,"nRequestsSurplus":0}"#;
const HUGE: &str = r#"{"cumVlm":"250000000000.0","nRequestsUsed":12,"nRequestsCap":250000010000,"nRequestsSurplus":0}"#;

/// Five answers, three of them live on mainnet and one on testnet, agree with the docs: ten
/// thousand to start and one more for every USDC traded. Throttled requests still count, so what
/// is used can run far past the cap and nothing is left, never a wrapped total; cancels have
/// `min(cap + 100000, cap * 2)`, so a throttled address can still cancel.
#[test]
fn the_cap_is_ten_thousand_and_one_more_for_every_usdc_traded() {
    for (said, volume, cap, remaining, cancels) in [
        (RATE_LIMIT_BUILDER, "30782.21", 40_782, 0, 81_564),
        (RATE_LIMIT_BUILDER_TESTNET, "14427.78", 24_427, 23_845, 48_854),
        (RATE_LIMIT_UNIFIED, "348945113.9599999785", 348_955_113, 348_943_501, 349_055_113),
        (TEN_DECIMALS, "1498565.3600000001", 1_508_565, 1_508_024, 1_608_565),
        (HUGE, "250000000000", 250_000_010_000, 250_000_009_988, 250_000_110_000),
    ] {
        let limit: RateLimit = read(said);
        assert_eq!(
            (limit.volume, limit.cap, RateLimit::earned_cap(limit.volume)),
            (dec(volume), cap, Some(cap)),
            "{said}"
        );
        assert_eq!(
            (limit.remaining(), limit.is_throttled(), limit.cancel_cap()),
            (remaining, remaining == 0, cancels),
            "{said}"
        );
    }
    assert_eq!(RateLimit::earned_cap(Dec::ZERO), Some(INITIAL_REQUESTS));
    assert_eq!(RateLimit::earned_cap(dec("-1")), None);
    assert_eq!((RateLimit::default().remaining(), THROTTLED_EVERY_MS), (0, 10_000), "one request every ten seconds");
}

/// More requests are earned by trading a USDC each or bought for 0.0005 USDC each from the perps
/// balance; a batch of `n` orders or cancels is one request to the IP and `n` to the address.
#[test]
fn requests_cost_what_the_docs_say() {
    assert_eq!(volume_for(5_000), dec("5000"));
    for (requests, cost) in [(10_000, "5"), (0, "0"), (1, "0.0005")] {
        assert_eq!(surplus_cost(requests), Some(dec(cost)), "{requests}");
    }
    for (batched, counted) in [(0, 1), (1, 1), (40, 40)] {
        assert_eq!(address_requests(batched), counted, "{batched}");
    }
}

/// The docs: 1000 open orders, one more for every 5M USDC of volume, at most 5000.
#[test]
fn open_orders_grow_with_volume_to_five_thousand() {
    for (volume, cap) in [
        ("0", 1_000),
        ("4999999.99", 1_000),
        ("5000000", 1_001),
        ("348945113.9599999785", 1_069),
        ("20000000000", 5_000),
        ("250000000000", 5_000),
    ] {
        assert_eq!(open_order_cap(dec(volume)), cap, "{volume}");
    }
}
