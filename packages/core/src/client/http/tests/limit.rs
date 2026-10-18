//! The docs' weights, the minute's meter and the gate a 429 closes, each handed the time.

use super::*;

#[test]
fn a_read_weighs_its_kind_and_its_items_and_an_action_its_batch() {
    for (kind, items, weight) in [
        ("l2Book", 0, 2),
        ("allMids", 0, 2),
        ("clearinghouseState", 0, 2),
        ("orderStatus", 0, 2),
        ("spotClearinghouseState", 0, 2),
        ("exchangeStatus", 0, 2),
        ("userRole", 0, 60),
        ("metaAndAssetCtxs", 500, 20),
        ("somethingNew", 0, 20),
        ("userFills", 19, 20),
        ("userFills", 45, 22),
        ("fundingHistory", 40, 22),
        ("candleSnapshot", 130, 22),
    ] {
        assert_eq!(read_weight(kind, items), weight, "{kind} with {items} items");
    }
    assert_eq!((per_items("candleSnapshot"), per_items("meta")), (Some(60), None));
    for (batched, weight) in [(0, 1), (1, 1), (39, 1), (40, 2), (85, 3)] {
        assert_eq!(action_weight(batched), weight, "{batched} batched");
    }
}

#[test]
fn reads_wait_once_the_minute_nears_the_limit() {
    let mut meter = Meter::default();
    for second in 0..47 {
        meter.record(second * 1_000, "metaAndAssetCtxs", 20);
    }
    // 940 spent: one more read of 20 fits under the soft limit, two do not.
    assert_eq!(meter.wait_for(47_000, 20), None);
    assert_eq!(meter.wait_for(47_000, 40), Some(13_000), "until the first read rolls off");
    assert_eq!(meter.wait_for(47_000, 60), Some(14_000));
    assert_eq!(meter.wait_for(47_000, 2_000), Some(WINDOW_MS), "more than a minute holds");
    assert_eq!(meter.wait_for(61_000, 40), None, "a minute on, room again");
}

#[test]
fn the_minute_adds_up_by_kind_heaviest_first_and_then_forgets() {
    let mut meter = Meter::default();
    meter.record(0, "l2Book", 2);
    meter.record(10, "l2Book", 2);
    meter.record(20, "userFills", 22);
    meter.record(30, "exchange", 1);
    meter.refuse(40);
    let spent = meter.spent(1_000);
    assert_eq!((spent.limit, spent.minute, spent.refused), (LIMIT, 27, 1));
    let kinds: Vec<(&str, u32, u32)> =
        spent.kinds.iter().map(|kind| (kind.name.as_str(), kind.count, kind.weight)).collect();
    assert_eq!(kinds, [("userFills", 1, 22), ("l2Book", 2, 4), ("exchange", 1, 1)]);
    let later = meter.spent(60_041);
    assert_eq!((later.minute, later.refused, later.kinds.len()), (0, 0, 0));
}

#[test]
fn a_strike_closes_the_gate_for_two_seconds_doubling_to_a_minute_unless_retry_after_asks_longer() {
    let mut gate = Gate::default();
    let mut now = 0;
    let mut waits = Vec::new();
    for _ in 0..7 {
        let ticket = gate.open(now).unwrap();
        waits.push(gate.struck(ticket, now, None));
        assert_eq!(gate.open(now + 500).map(|_| ()), Err(waits[waits.len() - 1] - 500), "closed to everything");
        now += waits[waits.len() - 1];
        assert_eq!(gate.cooldown_ms(now), 0);
    }
    assert_eq!(waits, [2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000]);
    for (retry_after, wait) in [(Some(10_000), 10_000), (Some(500), 2_000), (Some(120_000), 120_000)] {
        let mut gate = Gate::default();
        let ticket = gate.open(0).unwrap();
        assert_eq!(gate.struck(ticket, 0, retry_after), wait, "{retry_after:?}");
    }
}

#[test]
fn a_burst_is_one_strike_and_only_a_success_after_the_last_strike_clears_them() {
    let mut gate = Gate::default();
    let first = gate.open(0).unwrap();
    let second = gate.open(0).unwrap();
    assert_eq!(gate.struck(first, 0, None), 2_000);
    assert_eq!(gate.struck(second, 10, None), 1_990, "sent before the strike, so it does not double the wait");
    let early = gate.open(2_000).unwrap();
    let late = gate.open(2_000).unwrap();
    assert_eq!(gate.struck(late, 2_000, None), 4_000, "the next burst is the second strike");
    gate.passed(early);
    let next = gate.open(6_000).unwrap();
    assert_eq!(gate.struck(next, 6_000, None), 8_000, "the early success predates the strike and clears nothing");
    let ok = gate.open(14_000).unwrap();
    gate.passed(ok);
    let fresh = gate.open(14_000).unwrap();
    assert_eq!(gate.struck(fresh, 14_000, None), 2_000, "a success after the last strike starts over");
}
