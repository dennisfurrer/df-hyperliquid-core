use super::*;
use crate::fixtures::{
    BUILDER, RATE_LIMIT_BUILDER, RATE_LIMIT_BUILDER_TESTNET, RATE_LIMIT_UNIFIED, Recorded, address, block_on, dec, read,
};

/// hip4-tui's recorded answers (`net/src/tests/accounts.rs`): ten decimals of
/// volume, and a volume past what its own amount type counted.
const TEN_DECIMALS: &str =
    r#"{"cumVlm":"1498565.3600000001","nRequestsUsed":541,"nRequestsCap":1508565,"nRequestsSurplus":0}"#;
const HUGE: &str = r#"{"cumVlm":"250000000000.0","nRequestsUsed":12,"nRequestsCap":250000010000,"nRequestsSurplus":0}"#;

/// Five live answers, three of them mainnet and one testnet, all agree with
/// the docs: ten thousand to start, and one more for every USDC traded.
#[test]
fn the_cap_is_ten_thousand_and_one_more_for_every_usdc_traded() {
    for (said, volume, cap) in [
        (RATE_LIMIT_BUILDER, "30782.21", 40_782),
        (RATE_LIMIT_BUILDER_TESTNET, "14427.78", 24_427),
        (RATE_LIMIT_UNIFIED, "348945113.9599999785", 348_955_113),
        (TEN_DECIMALS, "1498565.3600000001", 1_508_565),
        (HUGE, "250000000000", 250_000_010_000),
    ] {
        let limit: RateLimit = read(said);
        assert_eq!((limit.volume, limit.cap), (dec(volume), cap), "{said}");
        assert_eq!(RateLimit::earned_cap(limit.volume), Some(cap), "{said}");
    }
    assert_eq!(RateLimit::earned_cap(Dec::ZERO), Some(INITIAL_REQUESTS));
}

/// hip4-tui's builder on mainnet, live: throttled requests still count, so
/// what is used runs far past the cap.
#[test]
fn a_budget_spent_past_its_cap_leaves_nothing_behind() {
    let spent: RateLimit = read(RATE_LIMIT_BUILDER);
    assert_eq!((spent.used, spent.remaining()), (673_028, 0), "and never a wrapped total");
    assert!(spent.is_throttled());
    let testnet: RateLimit = read(RATE_LIMIT_BUILDER_TESTNET);
    assert_eq!(testnet.remaining(), 23_845);
    assert!(!testnet.is_throttled());
    let fresh: RateLimit = read(TEN_DECIMALS);
    assert_eq!(fresh.remaining(), 1_508_024);
    assert_eq!(RateLimit::default().remaining(), 0);
    assert_eq!(THROTTLED_EVERY_MS, 10_000, "one request every ten seconds once spent");
}

/// The docs: cancels have `min(limit + 100000, limit * 2)`, so a throttled
/// address can still cancel.
#[test]
fn cancels_have_room_past_the_cap() {
    let small: RateLimit = read(RATE_LIMIT_BUILDER);
    assert_eq!(small.cancel_cap(), 81_564);
    let large: RateLimit = read(RATE_LIMIT_UNIFIED);
    assert_eq!(large.cancel_cap(), 349_055_113);
}

#[test]
fn more_requests_are_bought_by_trading_or_for_a_twentieth_of_a_cent_each() {
    assert_eq!(volume_for(5_000), dec("5000"), "one USDC traded for each");
    assert_eq!(surplus_cost(10_000), Some(dec("5")), "0.0005 USDC each, from the perps balance");
    assert_eq!(surplus_cost(0), Some(Dec::ZERO));
}

/// The docs: a batch of `n` orders or cancels is one request to the IP and
/// `n` to the address.
#[test]
fn a_batch_counts_each_order_against_the_address() {
    assert_eq!(address_requests(0), 1, "an action that batches nothing");
    assert_eq!(address_requests(1), 1);
    assert_eq!(address_requests(40), 40);
}

/// The docs: 1000 open orders, one more for every 5M USDC of volume, at
/// most 5000.
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

fn sample(seconds: u64, used: u64, cap: u64) -> Sample {
    Sample {
        at: 1_800_000_000_000 + seconds * 1_000,
        limit: RateLimit {
            volume: Dec::from_int(i64::try_from(cap - INITIAL_REQUESTS).unwrap()),
            used,
            cap,
            surplus: 0,
        },
    }
}

/// hip4-tui's `tui/src/tests/limits.rs`, in milliseconds.
#[test]
fn one_read_is_not_a_pace() {
    let mut budget = Budget::default();
    budget.took(sample(0, 100, 10_000));
    assert_eq!((budget.net_a_minute(), budget.runs_out_ms()), (None, None));
}

#[test]
fn the_pace_is_what_was_spent_a_minute_between_two_reads() {
    let mut budget = Budget::default();
    budget.took(sample(0, 100, 10_000));
    budget.took(sample(120, 340, 10_000));
    assert_eq!(budget.net_a_minute(), Some(120), "240 over two minutes");
}

#[test]
fn what_trading_earns_back_comes_off_the_pace() {
    let mut budget = Budget::default();
    budget.took(sample(0, 100, 10_000));
    budget.took(sample(60, 400, 10_250));
    assert_eq!(budget.net_a_minute(), Some(50), "300 spent, 250 earned");
    assert_eq!(budget.runs_out_ms().map(|ms| ms / 60_000), Some(197), "9,850 left at 50 a minute");
}

#[test]
fn a_bot_that_earns_more_than_it_spends_never_runs_out() {
    let mut budget = Budget::default();
    budget.took(sample(0, 100, 10_000));
    budget.took(sample(60, 400, 11_000));
    assert_eq!((budget.net_a_minute(), budget.runs_out_ms()), (Some(-700), None));
}

#[test]
fn a_read_seconds_after_another_keeps_the_pace_over_a_minute() {
    let mut budget = Budget::default();
    budget.took(sample(0, 100, 10_000));
    budget.took(sample(90, 190, 10_000));
    budget.took(sample(95, 200, 10_000));
    assert_eq!(budget.base.map(|base| base.at), Some(sample(0, 0, 10_000).at), "five seconds is not a minute");
    assert_eq!(budget.net_a_minute(), Some(63), "100 over 95 seconds");
}

#[test]
fn a_wait_runs_from_the_last_read_and_ends_a_minute_later() {
    let at = |seconds: u64| sample(seconds, 0, 10_000).at;
    assert_eq!(read_wait_ms(None, at(0)), None, "never read, so read now");
    assert_eq!(read_wait_ms(Some(at(0)), at(23)), Some(37_000));
    assert_eq!(read_wait_ms(Some(at(0)), at(60)), None);
    assert_eq!(read_wait_ms(Some(at(0)), at(61)), None);
    assert_eq!(READ_BATCH, 10);
}

#[test]
fn a_rate_limit_is_read_at_the_address() {
    let body = format!(r#"{{"type":"userRateLimit","user":"{BUILDER}"}}"#);
    assert_eq!(rate_limit_request(address(BUILDER)), body);
    let transport = Recorded::new(&[(&body, RATE_LIMIT_BUILDER)]);
    let limit = block_on(rate_limit(&transport, address(BUILDER))).unwrap();
    assert_eq!(limit.cap, 40_782);
}
