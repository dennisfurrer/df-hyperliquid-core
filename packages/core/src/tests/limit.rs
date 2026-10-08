use super::*;

#[test]
fn a_read_weighs_twenty_but_for_the_light_ones_and_user_role() {
    for kind in ["l2Book", "allMids", "clearinghouseState", "orderStatus", "spotClearinghouseState", "exchangeStatus"] {
        assert_eq!(base_weight(kind), 2, "{kind}");
    }
    assert_eq!(base_weight("userRole"), 60);
    assert_eq!(base_weight("metaAndAssetCtxs"), 20);
    assert_eq!(base_weight("somethingNew"), 20, "an unknown read is charged as an ordinary one");
}

#[test]
fn a_read_that_answers_with_a_list_weighs_one_more_per_twenty_items() {
    assert_eq!(read_weight("userFills", 19), 20);
    assert_eq!(read_weight("userFills", 45), 22);
    assert_eq!(read_weight("candleSnapshot", 130), 22, "candles are one more per sixty");
    assert_eq!(read_weight("metaAndAssetCtxs", 500), 20, "a read that is not charged per item");
    assert_eq!(per_items("fundingHistory"), Some(20));
    assert_eq!(per_items("meta"), None);
}

#[test]
fn an_action_weighs_one_and_one_more_per_forty_it_batches() {
    assert_eq!(action_weight(0), 1);
    assert_eq!(action_weight(1), 1);
    assert_eq!(action_weight(39), 1);
    assert_eq!(action_weight(40), 2);
    assert_eq!(action_weight(85), 3);
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
    assert_eq!(spent.minute, 27);
    assert_eq!(spent.limit, LIMIT);
    assert_eq!(spent.refused, 1);
    let kinds: Vec<(&str, u32, u32)> =
        spent.kinds.iter().map(|kind| (kind.name.as_str(), kind.count, kind.weight)).collect();
    assert_eq!(kinds, [("userFills", 1, 22), ("l2Book", 2, 4), ("exchange", 1, 1)]);
    let later = meter.spent(60_041);
    assert_eq!((later.minute, later.refused, later.kinds.len()), (0, 0, 0));
}

#[test]
fn a_429_closes_the_gate_for_two_seconds_and_each_strike_doubles_it_to_a_minute() {
    let mut gate = Gate::default();
    let mut now = 0;
    let mut waits = Vec::new();
    for _ in 0..7 {
        let ticket = gate.open(now).unwrap();
        waits.push(gate.struck(ticket, now, None));
        now += waits.last().copied().unwrap();
    }
    assert_eq!(waits, [2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000]);
}

#[test]
fn the_gate_refuses_everything_until_the_cooldown_ends() {
    let mut gate = Gate::default();
    let ticket = gate.open(0).unwrap();
    assert_eq!(gate.struck(ticket, 0, None), 2_000);
    assert_eq!(gate.open(500).map(|_| ()), Err(1_500));
    assert_eq!(gate.cooldown_ms(500), 1_500);
    assert!(gate.open(2_000).is_ok());
    assert_eq!(gate.cooldown_ms(2_000), 0);
}

#[test]
fn a_longer_retry_after_wins_and_a_shorter_one_does_not() {
    let mut gate = Gate::default();
    let ticket = gate.open(0).unwrap();
    assert_eq!(gate.struck(ticket, 0, Some(10_000)), 10_000);
    let mut gate = Gate::default();
    let ticket = gate.open(0).unwrap();
    assert_eq!(gate.struck(ticket, 0, Some(500)), 2_000);
}

#[test]
fn a_burst_is_one_strike() {
    let mut gate = Gate::default();
    let first = gate.open(0).unwrap();
    let second = gate.open(0).unwrap();
    assert_eq!(gate.struck(first, 0, None), 2_000);
    assert_eq!(gate.struck(second, 10, None), 1_990, "sent before the strike, so it does not double the wait");
    let third = gate.open(2_000).unwrap();
    assert_eq!(gate.struck(third, 2_000, None), 4_000, "the next burst is the second strike");
}

#[test]
fn a_success_clears_the_strikes_unless_a_strike_came_after_it_went_out() {
    let mut gate = Gate::default();
    let ticket = gate.open(0).unwrap();
    gate.struck(ticket, 0, None);
    let early = gate.open(2_000).unwrap();
    let late = gate.open(2_000).unwrap();
    gate.struck(late, 2_000, None);
    gate.passed(early);
    let next = gate.open(6_000).unwrap();
    assert_eq!(gate.struck(next, 6_000, None), 8_000, "the early success predates the strike and clears nothing");
    let ok = gate.open(14_000).unwrap();
    gate.passed(ok);
    let fresh = gate.open(14_000).unwrap();
    assert_eq!(gate.struck(fresh, 14_000, None), 2_000, "a success after the last strike starts over");
}
