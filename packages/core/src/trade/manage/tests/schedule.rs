//! The dead man's switch (armed with a time, disarmed with the key left out, not `null`) and the
//! noop, down to the bytes that are hashed.

use crate::utils::sign::L1Action;

use super::*;

const NOW: u64 = 1_790_000_000_000;

#[test]
fn the_switch_is_armed_with_a_time_and_disarmed_without_one() {
    assert_eq!(ScheduleCancel::at(NOW + 60_000).wire(), r#"{"type":"scheduleCancel","time":1790000060000}"#);
    assert_eq!(ScheduleCancel::disarm().wire(), r#"{"type":"scheduleCancel"}"#, "no key, rather than a null");
    assert_eq!(
        ScheduleCancel::disarm().msgpack(),
        [
            0x81, 0xa4, b't', b'y', b'p', b'e', 0xae, b's', b'c', b'h', b'e', b'd', b'u', b'l', b'e', b'C', b'a', b'n',
            b'c', b'e', b'l'
        ]
    );
    assert_eq!((ScheduleCancel::at(7).time(), ScheduleCancel::disarm().time()), (Some(7), None));
    assert_eq!(Noop.wire(), r#"{"type":"noop"}"#);
}

/// The exchange endpoint page: "the time must be at least 5 seconds after
/// the current time".
#[test]
fn armed_less_than_five_seconds_ahead_is_refused() {
    for (time, now, armed) in [
        (NOW + 5_000, NOW, Ok(ScheduleCancel::at(NOW + 5_000))),
        (NOW + 4_999, NOW, Err(TooSoon { time: NOW + 4_999, earliest: NOW + 5_000 })),
        (5, u64::MAX, Err(TooSoon { time: 5, earliest: u64::MAX })),
    ] {
        assert_eq!(ScheduleCancel::arm(time, now), armed, "{time} at {now}");
    }
    assert_eq!(
        ScheduleCancel::arm(NOW, NOW).unwrap_err().to_string(),
        "a scheduled cancel at 1790000000000 is under five seconds away; the earliest is 1790000005000"
    );
}
