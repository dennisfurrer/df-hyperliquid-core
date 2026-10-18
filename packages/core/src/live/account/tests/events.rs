use super::*;
use crate::live::Frame;
use crate::live::fixtures::{FIRST, SECOND, address, dec, mainnet, read, rebate};
use crate::markets::Side;

#[test]
fn user_events_come_on_the_user_channel_one_kind_a_frame() {
    assert_eq!(read(&mainnet("user.fills")), Frame::UserEvent(UserEvent::Fills(vec![rebate()])));
}

#[test]
fn recorded_user_events_read_as_their_kind() {
    let Frame::UserEvent(UserEvent::NonUserCancel(cancels)) = read(&mainnet("user.nonUserCancel")) else {
        panic!("cancels")
    };
    assert_eq!(
        cancels.iter().map(|cancel| (cancel.coin.as_str(), cancel.oid)).collect::<Vec<_>>(),
        [("ZEC", 569_031_862_179)]
    );

    let Frame::UserEvent(UserEvent::TwapHistory(history)) = read(&mainnet("user.twapHistory")) else {
        panic!("history")
    };
    assert_eq!(history.len(), 1);
    let started = &history[0];
    assert_eq!(
        (started.time, started.twap_id, started.status.status.as_str()),
        (1_791_481_178, Some(2_297_697), "activated")
    );
    assert_eq!(
        (started.state.coin.as_str(), started.state.side, started.state.sz, started.state.minutes),
        ("BTC", Side::Sell, dec("13.0"), 10)
    );

    let Frame::UserEvent(UserEvent::TwapSliceFills(slices)) = read(&mainnet("user.twapSliceFills")) else {
        panic!("slices")
    };
    assert_eq!(slices.len(), 1);
    assert_eq!((slices[0].twap_id, slices[0].fill.coin.as_str(), slices[0].fill.px), (2_257_646, "ENA", dec("0.1997")));
    assert_eq!(
        (slices[0].fill.sz, slices[0].fill.fee, slices[0].fill.tid),
        (dec("59.0"), dec("0.00458"), 805_092_930_237_458)
    );

    // On the hour: what a short position was paid.
    let Frame::UserEvent(UserEvent::Funding(funding)) = read(&mainnet("user.funding")) else { panic!("funding") };
    assert_eq!(
        (funding.time, funding.coin.as_str(), funding.usdc, funding.szi),
        (1_791_482_400_023, "ZEC", dec("0.082377"), dec("-5.8"))
    );
    assert_eq!((funding.funding_rate, funding.n_samples), (dec("0.0000125"), None));
}

/// No liquidation was recorded; this one follows the docs' `WsLiquidation` field for field, keys
/// in snake case as the docs write them. One frame holds one event, and a kind this build does not
/// know is kept as written.
#[test]
fn a_liquidation_reads_as_the_docs_type_it_and_an_unknown_event_is_kept() {
    let liquidation = read(
        r#"{"channel":"user","data":{"liquidation":{"lid":7,"liquidator":"0x42f1b36fb856769e075dfbf25938464fb3c92c70","liquidated_user":"0x78dce96236804d6958bf3c4598cf5efbc371629c","liquidated_ntl_pos":"1000.5","liquidated_account_value":"20.25"}}}"#,
    );
    let Frame::UserEvent(UserEvent::Liquidation(liquidation)) = liquidation else { panic!("a liquidation") };
    assert_eq!(
        (liquidation.lid, liquidation.liquidator, liquidation.liquidated_user),
        (7, address(FIRST), address(SECOND))
    );
    assert_eq!((liquidation.liquidated_ntl_pos, liquidation.liquidated_account_value), (dec("1000.5"), dec("20.25")));

    let unknown = read(r#"{"channel":"user","data":{"somethingNew":{"a":1.5}}}"#);
    assert_eq!(
        unknown,
        Frame::UserEvent(UserEvent::Other { key: "somethingNew".to_owned(), data: r#"{"a":1.5}"#.to_owned() })
    );
}
