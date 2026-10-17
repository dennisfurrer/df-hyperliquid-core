//! `fixtures/trade/exchange.json`: the venue's own example answers from its exchange endpoint
//! docs, statuses the venue sent, and the words `@nktkas/hyperliquid`'s `OrderResponse` lists.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::{ApiError, ExchangeResponse, Status, Transport, TransportError};
use crate::utils::{Dec, Json};

use super::*;
use crate::trade::order::Cloid;

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

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn accepted(text: &str) -> crate::client::Accepted {
    match ExchangeResponse::parse(text).unwrap() {
        ExchangeResponse::Ok(accepted) => accepted,
        ExchangeResponse::Err(reason) => panic!("refused: {reason}"),
    }
}

#[test]
fn every_order_status_the_venue_sends_is_read_as_itself() {
    assert_eq!(
        orders(&accepted(&answer("order_resting")), 1),
        Ok(vec![OrderAnswer::Resting { oid: 77_738_308, cloid: None }])
    );
    assert_eq!(
        orders(&accepted(&answer("order_filled")), 1),
        Ok(vec![OrderAnswer::Filled { oid: 77_747_314, total_sz: dec("0.02"), avg_px: dec("1891.4"), cloid: None }])
    );
    assert_eq!(
        orders(&accepted(&answer("order_refused")), 1),
        Ok(vec![OrderAnswer::Refused("Order must have minimum value of $10.".to_owned())])
    );
    assert_eq!(
        OrderAnswer::read(&status("resting_outcome")),
        Ok(OrderAnswer::Resting { oid: 543_186_691_772, cloid: None })
    );
    assert_eq!(
        OrderAnswer::read(&status("filled_outcome")),
        Ok(OrderAnswer::Filled { oid: 543_186_691_772, total_sz: dec("92"), avg_px: dec("0.27225"), cloid: None })
    );
    assert_eq!(OrderAnswer::read(&status("waiting_for_trigger")), Ok(OrderAnswer::WaitingForTrigger));
    assert_eq!(OrderAnswer::read(&status("waiting_for_fill")), Ok(OrderAnswer::WaitingForFill));
}

/// An average between two ticks is the venue's figure, kept exactly: never
/// brought onto a grid, never through a float.
/// The whole `response.data` is kept beside the statuses read from it, so a
/// field the statuses do not carry is still there to read.
#[test]
fn an_accepted_answer_keeps_its_data_as_the_venue_wrote_it() {
    let resting = accepted(&answer("order_resting"));
    assert_eq!(resting.data, Some(serde_json::json!({"statuses": [{"resting": {"oid": 77_738_308}}]})));
    let data = resting.data.as_ref().unwrap();
    assert_eq!(data["statuses"][0]["resting"]["oid"].as_u64(), Some(77_738_308));
    let twap = accepted(&answer("twap_running"));
    assert_eq!(twap.data, Some(serde_json::json!({"status": {"running": {"twapId": 77_738_308}}})));
    assert_eq!(accepted(&answer("default")).data, None, "a default answer has no data");
}

#[test]
fn an_average_price_is_kept_exactly() {
    let Ok(OrderAnswer::Filled { avg_px, total_sz, .. }) = OrderAnswer::read(&status("filled_between_ticks")) else {
        panic!("a fill")
    };
    assert_eq!((avg_px.to_wire(), total_sz.to_wire()), ("0.269935".to_owned(), "605".to_owned()));
}

#[test]
fn a_cloid_sent_comes_back_on_the_status() {
    let with = Status::Ok(
        serde_json::from_str(r#"{"resting":{"oid":7,"cloid":"0x0000000000000000000000000000abcd"}}"#).unwrap(),
    );
    assert_eq!(OrderAnswer::read(&with), Ok(OrderAnswer::Resting { oid: 7, cloid: Some(Cloid::from_u128(0xabcd)) }));
    assert_eq!(OrderAnswer::read(&with).unwrap().oid(), Some(7));
}

#[test]
fn a_status_this_build_does_not_know_is_never_read_as_placed() {
    for unfamiliar in [r#"{"somethingNew":{"oid":1}}"#, r#""success""#, r#""somethingElse""#] {
        let read = OrderAnswer::read(&Status::Ok(serde_json::from_str(unfamiliar).unwrap())).unwrap();
        assert!(matches!(read, OrderAnswer::Unknown(_)), "{unfamiliar}");
        assert_eq!(read.oid(), None);
        assert!(!read.is_placed(), "{unfamiliar}");
    }
    for broken in [r#"{"resting":{}}"#, r#"{"filled":{"oid":1,"totalSz":"1.0"}}"#, "7"] {
        assert!(OrderAnswer::read(&Status::Ok(serde_json::from_str(broken).unwrap())).is_err(), "{broken}");
    }
}

#[test]
fn a_cancel_is_taken_only_when_the_venue_says_success() {
    assert_eq!(cancels(&accepted(&answer("cancel_taken")), 1), Ok(vec![CancelAnswer::Cancelled]));
    assert_eq!(
        cancels(&accepted(&answer("cancel_refused")), 1),
        Ok(vec![CancelAnswer::Refused("Order was never placed, already canceled, or filled.".to_owned())])
    );
    let odd = accepted(r#"{"status":"ok","response":{"type":"cancel","data":{"statuses":["waitingForTrigger"]}}}"#);
    assert_eq!(cancels(&odd, 1), Ok(vec![CancelAnswer::Refused("waitingForTrigger".to_owned())]));
}

#[test]
fn a_twap_answers_with_the_id_it_runs_under() {
    assert_eq!(twap(&accepted(&answer("twap_running"))), Ok(TwapAnswer::Running { twap_id: 77_738_308 }));
    assert_eq!(
        twap(&accepted(&answer("twap_refused"))),
        Ok(TwapAnswer::Refused("Invalid TWAP duration: 1 min(s)".to_owned()))
    );
    assert_eq!(twap_cancel(&accepted(&answer("twap_cancel_taken"))), Ok(CancelAnswer::Cancelled));
    assert_eq!(
        twap_cancel(&accepted(&answer("twap_cancel_refused"))),
        Ok(CancelAnswer::Refused("TWAP was never placed, already canceled, or filled.".to_owned()))
    );
}

#[test]
fn a_modify_leverage_margin_schedule_cancel_or_noop_answers_default() {
    assert_eq!(taken(&accepted(&answer("default"))), Ok(()));
    let refused = accepted(&answer("order_refused"));
    assert_eq!(taken(&refused), Err(ApiError::Refused("Order must have minimum value of $10.".to_owned())));
}

/// The venue's error responses page: an error decided by the payload alone
/// comes back once for the whole batch, and stands for every order in it.
#[test]
fn one_error_for_a_batch_refuses_every_order_in_it() {
    let one = accepted(&answer("order_refused"));
    let read = orders(&one, 3).unwrap();
    assert_eq!(read, vec![OrderAnswer::Refused("Order must have minimum value of $10.".to_owned()); 3]);
    let cancelled = accepted(&answer("cancel_refused"));
    assert_eq!(cancels(&cancelled, 2).unwrap().len(), 2);
}

#[test]
fn a_status_count_that_does_not_match_is_unreadable() {
    let one = accepted(&answer("order_resting"));
    assert!(matches!(orders(&one, 2), Err(ApiError::Unreadable { .. })), "one resting for two sent is not a batch");
    assert!(matches!(cancels(&accepted(&answer("cancel_taken")), 2), Err(ApiError::Unreadable { .. })));
    assert!(matches!(orders(&accepted(&answer("default")), 1), Err(ApiError::Unreadable { .. })), "no statuses at all");
}

#[test]
fn a_refusal_of_the_whole_action_is_the_venue_s_words() {
    assert_eq!(
        read(&answer("refused_whole")),
        Err(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))
    );
    assert!(read(&answer("order_resting")).is_ok());
}

/// Answers `/exchange` from the fixture, keeping what was posted.
struct Recorded {
    answer: String,
    sent: RefCell<Vec<(String, String)>>,
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(Ok(self.answer.clone()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn each_send_posts_once_to_exchange_and_reads_its_answer() {
    let transport = Recorded { answer: answer("order_filled"), sent: RefCell::new(Vec::new()) };
    let filled = block_on(send_orders(&transport, "{}".to_owned(), 1)).unwrap();
    assert!(filled[0].is_placed());
    assert_eq!(transport.sent.borrow().as_slice(), [("/exchange".to_owned(), "{}".to_owned())]);
    let refusing = Recorded { answer: answer("refused_whole"), sent: RefCell::new(Vec::new()) };
    assert!(matches!(block_on(send_cancels(&refusing, "{}".to_owned(), 1)), Err(ApiError::Refused(_))));
    let twap = Recorded { answer: answer("twap_running"), sent: RefCell::new(Vec::new()) };
    assert_eq!(block_on(send_twap(&twap, "{}".to_owned())), Ok(TwapAnswer::Running { twap_id: 77_738_308 }));
    let cancel = Recorded { answer: answer("twap_cancel_taken"), sent: RefCell::new(Vec::new()) };
    assert_eq!(block_on(send_twap_cancel(&cancel, "{}".to_owned())), Ok(CancelAnswer::Cancelled));
    let default = Recorded { answer: answer("default"), sent: RefCell::new(Vec::new()) };
    assert_eq!(block_on(send_taken(&default, "{}".to_owned())), Ok(()));
}
