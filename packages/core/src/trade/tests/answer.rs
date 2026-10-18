//! `fixtures/trade/exchange.json`: the venue's own example answers from its exchange endpoint
//! docs, statuses the venue sent, and the words `@nktkas/hyperliquid`'s `OrderResponse` lists.
//! Posting them through a client is in `tests/mocked/trade`.

use crate::client::{Accepted, ApiError, ExchangeResponse, Status};
use crate::utils::{Dec, Json};

use super::*;
use crate::trade::orders::Cloid;

const EXCHANGE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/exchange.json"));

fn fixture(list: &str, name: &str, key: &str) -> String {
    let file = Json::parse(EXCHANGE).unwrap();
    let rows = file.get(list).and_then(Json::as_array).unwrap();
    let row = rows.iter().find(|row| row.get("name").and_then(Json::as_str) == Some(name)).unwrap();
    row.get(key).and_then(Json::as_str).unwrap().to_owned()
}

fn answer(name: &str) -> String {
    fixture("answers", name, "answer")
}

fn status(name: &str) -> Status {
    Status::Ok(serde_json::from_str(&fixture("statuses", name, "status")).unwrap())
}

fn written(json: &str) -> Status {
    Status::Ok(serde_json::from_str(json).unwrap())
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn accepted(text: &str) -> Accepted {
    match ExchangeResponse::parse(text).unwrap() {
        ExchangeResponse::Ok(accepted) => accepted,
        ExchangeResponse::Err(reason) => panic!("refused: {reason}"),
    }
}

/// An average between two ticks is the venue's figure, kept exactly: never brought onto a grid,
/// never through a float. A cloid sent comes back on the status.
#[test]
fn every_order_status_the_venue_sends_is_read_as_itself() {
    let filled = |oid: u64, total_sz: &str, avg_px: &str| OrderAnswer::Filled {
        oid,
        total_sz: dec(total_sz),
        avg_px: dec(avg_px),
        cloid: None,
    };
    for (name, read) in [
        ("order_resting", OrderAnswer::Resting { oid: 77_738_308, cloid: None }),
        ("order_filled", filled(77_747_314, "0.02", "1891.4")),
        ("order_refused", OrderAnswer::Refused("Order must have minimum value of $10.".to_owned())),
    ] {
        assert_eq!(orders(&accepted(&answer(name)), 1), Ok(vec![read]), "{name}");
    }
    for (name, read) in [
        ("resting_outcome", OrderAnswer::Resting { oid: 543_186_691_772, cloid: None }),
        ("filled_outcome", filled(543_186_691_772, "92", "0.27225")),
        ("filled_between_ticks", filled(1, "605", "0.269935")),
        ("waiting_for_trigger", OrderAnswer::WaitingForTrigger),
        ("waiting_for_fill", OrderAnswer::WaitingForFill),
    ] {
        let status = OrderAnswer::read(&status(name)).unwrap();
        assert!(status.is_placed() && status.refusal().is_none(), "{name}");
        assert_eq!(status, read, "{name}");
    }
    let Ok(OrderAnswer::Filled { avg_px, .. }) = OrderAnswer::read(&status("filled_between_ticks")) else { panic!() };
    assert_eq!(avg_px.to_wire(), "0.269935");
    let with = OrderAnswer::read(&written(r#"{"resting":{"oid":7,"cloid":"0x0000000000000000000000000000abcd"}}"#));
    assert_eq!(with, Ok(OrderAnswer::Resting { oid: 7, cloid: Some(Cloid::from_u128(0xabcd)) }));
    assert_eq!(with.unwrap().oid(), Some(7));
}

#[test]
fn a_status_this_build_does_not_know_is_never_read_as_placed() {
    for unfamiliar in [r#"{"somethingNew":{"oid":1}}"#, r#""success""#, r#""somethingElse""#] {
        let read = OrderAnswer::read(&written(unfamiliar)).unwrap();
        assert!(matches!(read, OrderAnswer::Unknown(_)), "{unfamiliar}");
        assert_eq!((read.oid(), read.is_placed()), (None, false), "{unfamiliar}");
    }
    for broken in [r#"{"resting":{}}"#, r#"{"filled":{"oid":1,"totalSz":"1.0"}}"#, "7"] {
        assert!(OrderAnswer::read(&written(broken)).is_err(), "{broken}");
    }
}

/// A cancel, a TWAP's cancel, a leverage, margin, `scheduleCancel` or `noop` is taken only on the
/// venue's word; a TWAP answers with the id it runs under.
#[test]
fn a_cancel_twap_or_plain_action_is_taken_only_when_the_venue_says_so() {
    assert_eq!(cancels(&accepted(&answer("cancel_taken")), 1), Ok(vec![CancelAnswer::Cancelled]));
    assert_eq!(
        cancels(&accepted(&answer("cancel_refused")), 1),
        Ok(vec![CancelAnswer::Refused("Order was never placed, already canceled, or filled.".to_owned())])
    );
    let odd = accepted(r#"{"status":"ok","response":{"type":"cancel","data":{"statuses":["waitingForTrigger"]}}}"#);
    assert_eq!(cancels(&odd, 1), Ok(vec![CancelAnswer::Refused("waitingForTrigger".to_owned())]));
    assert_eq!(CancelAnswer::read(&written(r#"{"x":1}"#)), CancelAnswer::Refused(r#"{"x":1}"#.to_owned()));

    assert_eq!(twap(&accepted(&answer("twap_running"))), Ok(TwapAnswer::Running { twap_id: 77_738_308 }));
    assert_eq!(
        twap(&accepted(&answer("twap_refused"))),
        Ok(TwapAnswer::Refused("Invalid TWAP duration: 1 min(s)".to_owned()))
    );
    assert!(matches!(twap(&accepted(&answer("twap_cancel_taken"))), Err(ApiError::Unreadable { .. })));
    assert!(matches!(twap(&accepted(&answer("default"))), Err(ApiError::Unreadable { .. })), "no status at all");
    assert_eq!(twap_cancel(&accepted(&answer("twap_cancel_taken"))), Ok(CancelAnswer::Cancelled));
    assert_eq!(
        twap_cancel(&accepted(&answer("twap_cancel_refused"))),
        Ok(CancelAnswer::Refused("TWAP was never placed, already canceled, or filled.".to_owned()))
    );

    assert_eq!(taken(&accepted(&answer("default"))), Ok(()));
    let refused = accepted(&answer("order_refused"));
    assert_eq!(taken(&refused), Err(ApiError::Refused("Order must have minimum value of $10.".to_owned())));
    assert_eq!(
        read(&answer("refused_whole")),
        Err(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))
    );
    assert!(read(&answer("order_resting")).is_ok());
}

/// The venue's error responses page: an error decided by the payload alone comes back once for the
/// whole batch, and stands for every order in it. Any other count is unreadable, never a partial
/// success.
#[test]
fn a_batch_is_answered_a_status_an_item_or_one_error_for_all() {
    let one = accepted(&answer("order_refused"));
    assert_eq!(
        orders(&one, 3).unwrap(),
        vec![OrderAnswer::Refused("Order must have minimum value of $10.".to_owned()); 3]
    );
    assert_eq!(cancels(&accepted(&answer("cancel_refused")), 2).unwrap().len(), 2);
    let unreadable = |read: Result<(), ApiError>| matches!(read, Err(ApiError::Unreadable { .. }));
    assert!(unreadable(orders(&accepted(&answer("order_resting")), 2).map(drop)), "one resting for two sent");
    assert!(unreadable(cancels(&accepted(&answer("cancel_taken")), 2).map(drop)), "one success for two sent");
    assert!(unreadable(orders(&accepted(&answer("default")), 1).map(drop)), "no statuses at all");
}
