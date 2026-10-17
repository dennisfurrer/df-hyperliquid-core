use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::TransportError;
use crate::utils::Dec;

use super::*;

/// Live answers, mainnet, 2026-10-06.
const SPOT: &str = r#"{"balances":[{"coin":"USDC","token":0,"total":"8155.50418728","hold":"0.0","entryNtl":"0.0"},{"coin":"PURR","token":1,"total":"62.97481","hold":"0.0","entryNtl":"14.6492003"},{"coin":"HYPE","token":150,"total":"0.00000004","hold":"0.0","entryNtl":"0.00000359"}]}"#;
const PERP: &str = r#"{"marginSummary":{"accountValue":"41085992.9213469997","totalNtlPos":"0.0","totalRawUsd":"41085992.9213469997","totalMarginUsed":"477.867397"},"crossMarginSummary":{"accountValue":"41085515.0539499968","totalNtlPos":"0.0","totalRawUsd":"41085515.0539499968","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"41085515.0539499968","assetPositions":[],"time":1791304588904}"#;
/// A unified account holding HIP-4 outcome rows, which have no `token`, and
/// the pooled extra `tokenToAvailableAfterMaintenance` (live, 2026-10-06).
const SPOT_WITH_OUTCOMES: &str = r#"{"balances":[{"coin":"USDC","token":0,"total":"11.515057","hold":"11.515057","entryNtl":"0.0"},{"coin":"USDE","token":235,"total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92260","total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92261","total":"0.0","hold":"0.0","entryNtl":"0.0"}],"tokenToAvailableAfterMaintenance":[[0,"8.554471"]]}"#;
const FRESH_PERP: &str = r#"{"marginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"0.0","assetPositions":[],"time":1791305116226}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn user() -> Address {
    "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap()
}

fn spot(usdc_total: &str, usdc_hold: &str) -> SpotState {
    SpotState {
        balances: vec![SpotBalance {
            coin: "USDC".to_owned(),
            token: Some(0),
            total: dec(usdc_total),
            hold: dec(usdc_hold),
            entry_ntl: Dec::ZERO,
            spot_hold: None,
            borrowed: None,
            supplied: None,
            ltv: None,
        }],
        ..SpotState::default()
    }
}

fn perp(withdrawable: &str) -> PerpState {
    PerpState {
        margin_summary: MarginSummary { account_value: dec(withdrawable), ..MarginSummary::default() },
        withdrawable: dec(withdrawable),
        ..PerpState::default()
    }
}

#[test]
fn spot_state_reads_the_live_answer_and_finds_usdc_by_index() {
    let state: SpotState = serde_json::from_str(SPOT).unwrap();
    assert_eq!(state.balances.len(), 3);
    let usdc = state.usdc().unwrap();
    assert_eq!(
        (usdc.coin.as_str(), usdc.token, usdc.total, usdc.hold),
        ("USDC", Some(0), dec("8155.50418728"), Dec::ZERO)
    );
    assert_eq!(state.balance(150).unwrap().total, dec("0.00000004"));
    let empty: SpotState = serde_json::from_str(r#"{"balances":[]}"#).unwrap();
    assert_eq!(empty.usdc(), None);
}

/// Portfolio margin, live 2026-10-06: a negative `hold` and a `spotHold`
/// holding the whole (tiny) USDC balance. `total - hold` would read $269.38
/// as free; nothing is.
#[test]
fn a_negative_hold_never_makes_money_available() {
    let state: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"0.55817531","hold":"-268.81821068","entryNtl":"0.0","spotHold":"0.55817531","ltv":"0.0","borrowed":"89.3303277"},{"coin":"HYPE","token":150,"total":"6.02901107","hold":"1.50143146","entryNtl":"0.0","ltv":"0.65","supplied":"6.02901107"}]}"#,
    )
    .unwrap();
    let usdc = state.usdc().unwrap();
    assert_eq!(usdc.available(), Some(Dec::ZERO));
    assert_eq!(usdc.borrowed, Some(dec("89.3303277")));
    let hype = state.balance(150).unwrap();
    assert_eq!(hype.available(), Some(dec("4.52757961")));
    assert_eq!(hype.supplied, Some(dec("6.02901107")));

    let free: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"50.0","hold":"-10.0","entryNtl":"0.0"}]}"#,
    )
    .unwrap();
    assert_eq!(free.usdc().unwrap().available(), Some(dec("50")), "never above total");
    let owed: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"-18.9","hold":"-18.9","entryNtl":"0.0"}]}"#,
    )
    .unwrap();
    assert_eq!(owed.usdc().unwrap().available(), Some(Dec::ZERO), "never below zero");
}

#[test]
fn outcome_rows_without_a_token_do_not_break_the_read() {
    let state: SpotState = serde_json::from_str(SPOT_WITH_OUTCOMES).unwrap();
    assert_eq!(state.balances.len(), 4);
    assert_eq!(state.balances[2].token, None);
    let usdc = state.usdc().unwrap();
    assert_eq!((usdc.total, usdc.available()), (dec("11.515057"), Some(Dec::ZERO)));
}

#[test]
fn perp_state_reads_what_it_needs_and_ignores_the_rest() {
    let state: PerpState = serde_json::from_str(PERP).unwrap();
    assert_eq!(state.withdrawable, dec("41085515.0539499968"));
    assert_eq!(state.margin_summary.account_value, dec("41085992.9213469997"));
    let fresh: PerpState = serde_json::from_str(FRESH_PERP).unwrap();
    assert_eq!(fresh.withdrawable, Dec::ZERO);
}

#[test]
fn every_mode_reads_from_a_bare_string_and_only_two_pool() {
    for (said, mode, pooled) in [
        ("\"default\"", Abstraction::Default, false),
        ("\"disabled\"", Abstraction::Disabled, false),
        ("\"unifiedAccount\"", Abstraction::UnifiedAccount, true),
        ("\"portfolioMargin\"", Abstraction::PortfolioMargin, true),
        ("\"dexAbstraction\"", Abstraction::DexAbstraction, false),
        ("\"someNewMode\"", Abstraction::Unknown("someNewMode".to_owned()), false),
    ] {
        let read: Abstraction = serde_json::from_str(said).unwrap();
        assert_eq!(read, mode, "{said}");
        assert_eq!(read.is_pooled(), pooled, "{said}");
        assert_eq!(format!("\"{read}\""), said);
    }
    assert!(serde_json::from_str::<Abstraction>("null").is_err());
}

#[test]
fn a_standard_account_counts_spot_and_the_perp_balance() {
    let dollars = Dollars::new(&Abstraction::Default, &spot("100.5", "20"), &perp("12.345678")).unwrap();
    assert_eq!(
        dollars,
        Dollars {
            spot_total: dec("100.5"),
            spot_available: dec("80.5"),
            perp_withdrawable: dec("12.345678"),
            total: dec("112.845678"),
            available: dec("92.845678"),
        }
    );
}

#[test]
fn a_pooled_account_counts_spot_alone() {
    for mode in [Abstraction::UnifiedAccount, Abstraction::PortfolioMargin] {
        let dollars = Dollars::new(&mode, &spot("100.5", "0.5"), &perp("99")).unwrap();
        assert_eq!(dollars.perp_withdrawable, Dec::ZERO, "{mode}");
        assert_eq!((dollars.total, dollars.available), (dec("100.5"), dec("100")), "{mode}");
    }
}

#[test]
fn an_account_without_spot_usdc_has_its_perp_balance_or_nothing() {
    let none = SpotState::default();
    let standard = Dollars::new(&Abstraction::Disabled, &none, &perp("7")).unwrap();
    assert_eq!((standard.spot_total, standard.total, standard.available), (Dec::ZERO, dec("7"), dec("7")));
    let pooled = Dollars::new(&Abstraction::UnifiedAccount, &none, &perp("7")).unwrap();
    assert_eq!(pooled.total, Dec::ZERO);
}

/// Answers each read by its `type` and keeps what was asked. Nothing leaves
/// the process.
struct Recorded {
    sent: RefCell<Vec<(String, String)>>,
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        let answer = match serde_json::from_str::<serde_json::Value>(&body).unwrap()["type"].as_str() {
            Some("userAbstraction") => "\"default\"",
            Some("spotClearinghouseState") => SPOT,
            Some("clearinghouseState") => PERP,
            _ => "null",
        };
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(Ok(answer.to_owned()))
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
fn the_reads_post_to_info_with_the_user_lowercase() {
    let transport = Recorded { sent: RefCell::new(Vec::new()) };
    let dollars = block_on(dollars(&transport, user())).unwrap();
    assert_eq!(dollars.total, dec("8155.50418728").checked_add(dec("41085515.0539499968")).unwrap());
    let sent = transport.sent.borrow();
    let user = r#""user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414""#;
    assert_eq!(
        sent.as_slice(),
        [
            ("/info".to_owned(), format!(r#"{{"type":"userAbstraction",{user}}}"#)),
            ("/info".to_owned(), format!(r#"{{"type":"spotClearinghouseState",{user}}}"#)),
            ("/info".to_owned(), format!(r#"{{"type":"clearinghouseState",{user}}}"#)),
        ]
    );
}
