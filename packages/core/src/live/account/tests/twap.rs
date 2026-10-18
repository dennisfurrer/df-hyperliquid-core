use super::*;
use crate::live::Frame;
use crate::live::fixtures::{address, dec, mainnet, read};

#[test]
fn twap_states_slices_and_history_read_as_recorded() {
    let state = TwapState {
        coin: "@107".to_owned(),
        user: address("0x51a4cee5e31cc8f3bef1564b7a2d0005e2775a98"),
        side: Side::Buy,
        sz: dec("5000.0"),
        executed_sz: dec("20.8"),
        executed_ntl: dec("1738.48367"),
        minutes: 10_080,
        reduce_only: false,
        randomize: true,
        timestamp: 1_791_478_452_792,
    };
    let Frame::TwapStates(states) = read(&mainnet("twapStates")) else { panic!("twaps") };
    assert_eq!(
        (states.dex.as_str(), states.user, states.states.as_slice()),
        ("", state.user, [(2_297_432, state)].as_slice())
    );
    let Frame::TwapStates(none) = read(&mainnet("twapStates.empty")) else { panic!("twaps") };
    assert!(none.states.is_empty());

    let Frame::TwapSliceFills(slices) = read(&mainnet("userTwapSliceFills")) else { panic!("slices") };
    assert_eq!(
        (slices.user, slices.is_snapshot, slices.fills.len()),
        (address("0xbe3f79ae0ab3294aaa3230c1155e912c05b6a55b"), false, 1)
    );
    let slice = &slices.fills[0];
    assert_eq!((slice.twap_id, slice.fill.px, slice.fill.twap_id), (2_296_264, dec("80832.0"), None));
    assert_eq!(slice.fill.hash, format!("0x{}", "0".repeat(64)), "a TWAP slice has a zero hash");

    let Frame::TwapHistory(history) = read(&mainnet("userTwapHistory")) else { panic!("history") };
    assert_eq!((history.is_snapshot, history.history.len()), (true, 3));
    let started = &history.history[0];
    assert_eq!(
        (started.time, started.status.status.as_str(), started.status.description.as_deref()),
        (1_791_401_487, "activated", None)
    );
    assert_eq!(
        (started.twap_id, started.state.sz, started.state.coin.as_str()),
        (Some(2_294_081), dec("2831.96"), "HYPE")
    );
    let stopped = &history.history[1];
    assert_eq!(
        (stopped.status.status.as_str(), stopped.state.executed_sz, stopped.state.executed_ntl),
        ("terminated", dec("0.14"), dec("12.355"))
    );
    let Frame::TwapSliceFills(none) = read(&mainnet("userTwapSliceFills.empty")) else { panic!("slices") };
    assert!(none.is_snapshot && none.fills.is_empty());
    let Frame::TwapHistory(none) = read(&mainnet("userTwapHistory.empty")) else { panic!("history") };
    assert!(none.is_snapshot && none.history.is_empty());
}
