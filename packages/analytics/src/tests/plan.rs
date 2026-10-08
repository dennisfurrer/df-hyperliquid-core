//! hip4-tui's `engine/src/tests/payout/plan.rs`.

use hl_core::{Address, Dec};

use super::*;
use crate::payout::ledger::{self, Status};

const TREASURY: &str = "0x7661d1f77f4f0e2c9a6c0c1d9e8b7a6f5d4c3b2a";
const OTHER: &str = "0x9999999999999999999999999999999999999999";

fn at(hex: &str) -> Address {
    hex.parse().unwrap()
}

fn wallet(n: u8) -> Address {
    Address::from_bytes([n; 20])
}

fn now() -> At {
    At::parse("2026-09-10T12:00:00Z").unwrap()
}

fn usdc(said: &str) -> Dec {
    money::parse(said).unwrap()
}

fn batch(id: &str, treasury: &str, amounts: &[&str]) -> String {
    let rows: Vec<String> = amounts
        .iter()
        .enumerate()
        .map(|(at, said)| {
            format!(
                r#"{{"wallet":"{}","amount_usdc":"{said}","line":{}}}"#,
                wallet(u8::try_from(at + 1).unwrap()).to_hex(),
                at + 2
            )
        })
        .collect();
    format!(
        r#"{{"type":"batch_created","at":"2026-09-01T10:00:00.000Z","batch_id":"{id}",
        "source":"payouts.csv","treasury":"{treasury}","rows":[{}]}}"#,
        rows.join(",")
    )
    .replace('\n', "")
}

fn state(lines: &[String]) -> State {
    ledger::fold(&ledger::read(&lines.join("\n")))
}

fn plain() -> (State, Dec) {
    (state(&[batch("b1", TREASURY, &["10", "20", "30"])]), usdc("60"))
}

fn open() -> Config {
    Config { max_batch_total_usdc: None, max_csv_total_usdc: None, duplicate_window_days: 0, ..Config::default() }
}

#[test]
fn every_pending_row_is_owed_and_sendable_by_default() {
    let (state, total) = plain();
    let plan = plan(&state, "b1", &open(), total, None, None, now());
    assert_eq!(plan.payable.len(), 3);
    assert_eq!(plan.sendable.len(), 3);
    assert_eq!(money::wire(plan.outstanding), "60");
    assert!(!plan.is_blocked());
}

#[test]
fn a_row_below_the_floor_is_skipped_rather_than_sent_as_dust() {
    let (state, total) = plain();
    let config = Config { min_payout_usdc: Some("15".to_owned()), ..open() };
    let plan = plan(&state, "b1", &config, total, None, None, now());
    assert_eq!(plan.diverted_by(Diverted::Dust), vec![0]);
    assert_eq!(plan.sendable, vec![1, 2]);
    assert_eq!(money::wire(plan.outstanding), "50");
    assert!(!plan.is_blocked(), "dust diverts a row, it does not stop a run");
}

#[test]
fn a_row_above_the_ceiling_is_parked_rather_than_sent() {
    let (state, total) = plain();
    let config = Config { max_single_payout_usdc: Some("25".to_owned()), ..open() };
    let plan = plan(&state, "b1", &config, total, None, None, now());
    assert_eq!(plan.diverted_by(Diverted::Oversized), vec![2]);
    assert_eq!(money::wire(plan.outstanding), "30");
}

#[test]
fn a_file_over_the_whole_file_ceiling_is_refused_outright() {
    let (state, total) = plain();
    let config = Config { max_csv_total_usdc: Some("50".to_owned()), ..open() };
    let plan = plan(&state, "b1", &config, total, None, None, now());
    assert!(plan.is_blocked());
    assert!(plan.blockers[0].contains("over the 50 ceiling"));
}

#[test]
fn a_declared_total_that_is_not_the_files_total_refuses_it() {
    let (state, total) = plain();
    let wrong = plan(&state, "b1", &open(), total, Some(Expected("59")), None, now());
    assert!(wrong.is_blocked());
    assert!(wrong.blockers[0].contains("59"));
    let right = plan(&state, "b1", &open(), total, Some(Expected("60")), None, now());
    assert!(!right.is_blocked());
}

#[test]
fn a_declared_total_that_is_not_a_number_refuses_rather_than_passing() {
    let (state, total) = plain();
    assert!(plan(&state, "b1", &open(), total, Some(Expected("sixty")), None, now()).is_blocked());
}

#[test]
fn a_treasury_short_of_hype_refuses_the_run() {
    let (state, total) = plain();
    let config = Config { min_hype_balance: Some("1".to_owned()), ..open() };
    let thin = Treasury { spot_hype: usdc("0.5"), ..Treasury::default() };
    assert!(plan(&state, "b1", &config, total, None, Some(thin), now()).is_blocked());
    let held = Treasury { spot_hype: usdc("2"), ..Treasury::default() };
    assert!(!plan(&state, "b1", &config, total, None, Some(held), now()).is_blocked());
}

#[test]
fn an_unread_balance_is_not_a_blocker_because_the_slice_check_reads_again() {
    let (state, total) = plain();
    let config = Config { min_hype_balance: Some("1".to_owned()), ..open() };
    assert!(
        !plan(&state, "b1", &config, total, None, None, now()).is_blocked(),
        "the per-slice pre-flight stops it instead"
    );
}

#[test]
fn a_cap_that_will_not_read_blocks_the_run_rather_than_reading_as_no_cap() {
    let (state, total) = plain();
    let config = Config { max_csv_total_usdc: Some("lots".to_owned()), ..open() };
    assert!(plan(&state, "b1", &config, total, None, None, now()).is_blocked());
}

#[test]
fn a_sent_row_is_no_longer_owed_and_a_claimed_one_is_never_resent() {
    let state = state(&[
        batch("b1", TREASURY, &["10", "20", "30"]),
        format!(
            r#"{{"type":"row_sent","at":"2026-09-02T10:00:00.000Z","batch_id":"b1","wallet":"{}","run_id":"r1"}}"#,
            wallet(1).to_hex()
        ),
        format!(
            r#"{{"type":"row_claimed","at":"2026-09-02T10:00:00.000Z","batch_id":"b1","wallet":"{}","run_id":"r1"}}"#,
            wallet(2).to_hex()
        ),
    ]);
    let plan = plan(&state, "b1", &open(), usdc("60"), None, None, now());
    assert_eq!(plan.sendable, vec![2], "only the untouched row");
    assert_eq!(state.rows("b1")[1].status, Status::Submitting);
}

#[test]
fn a_failed_row_stops_being_owed_once_it_runs_out_of_attempts() {
    let mut lines = vec![batch("b1", TREASURY, &["10"])];
    for turn in 0..2 {
        lines.push(format!(
            r#"{{"type":"row_claimed","at":"2026-09-02T10:0{turn}:00.000Z","batch_id":"b1","wallet":"{}","run_id":"r1"}}"#,
            wallet(1).to_hex()
        ));
        lines.push(format!(
            r#"{{"type":"row_failed","at":"2026-09-02T10:0{turn}:01.000Z","batch_id":"b1","wallet":"{}","run_id":"r1","error":"no"}}"#,
            wallet(1).to_hex()
        ));
    }
    let state = state(&lines);
    let under = Config { max_attempts: 3, ..open() };
    assert_eq!(plan(&state, "b1", &under, usdc("10"), None, None, now()).sendable.len(), 1);
    let spent = Config { max_attempts: 2, ..open() };
    assert!(plan(&state, "b1", &spent, usdc("10"), None, None, now()).sendable.is_empty());
}

#[test]
fn a_wallet_paid_the_same_amount_lately_is_flagged_with_the_budget_that_paid_it() {
    let state = state(&[
        format!(
            r#"{{"type":"batch_created","at":"2026-09-08T10:00:00.000Z","batch_id":"old",
            "source":"last.csv","treasury":"{OTHER}","account_label":"[JB] Marketing",
            "rows":[{{"wallet":"{}","amount_usdc":"10","line":2}}]}}"#,
            wallet(1).to_hex()
        )
        .replace('\n', ""),
        format!(
            r#"{{"type":"row_sent","at":"2026-09-08T10:01:00.000Z","batch_id":"old","wallet":"{}","run_id":"r1"}}"#,
            wallet(1).to_hex()
        ),
        batch("b1", TREASURY, &["10", "20"]),
    ]);
    let config = Config { duplicate_window_days: 7, ..open() };
    let plan = plan(&state, "b1", &config, usdc("30"), None, None, now());
    assert_eq!(plan.repeats.len(), 1);
    assert_eq!(plan.repeats[0].at, 0);
    assert_eq!(plan.repeats[0].earlier_account, "[JB] Marketing");
    assert_eq!(plan.repeats[0].when, At::parse("2026-09-08T10:01:00Z").unwrap());
    assert_eq!(
        plan.repeats_elsewhere("[ND] Founder Rewards"),
        1,
        "paid out of another budget, which is the mix-up worth stopping for"
    );
    assert_eq!(plan.repeats_elsewhere("[JB] Marketing"), 0);
    assert!(!plan.is_blocked(), "a repeat warns, it does not refuse");
}

#[test]
fn looking_back_can_be_turned_off_entirely() {
    let (state, total) = plain();
    assert!(plan(&state, "b1", &open(), total, None, None, now()).repeats.is_empty());
}

#[test]
fn the_rows_are_cut_into_slices_no_larger_than_the_cap() {
    let (state, total) = plain();
    let narrow = Config { max_batch_total_usdc: Some("25".to_owned()), ..open() };
    assert_eq!(plan(&state, "b1", &narrow, total, None, None, now()).slices, vec![vec![0], vec![1], vec![2]]);
    let wider = Config { max_batch_total_usdc: Some("35".to_owned()), ..open() };
    assert_eq!(plan(&state, "b1", &wider, total, None, None, now()).slices, vec![vec![0, 1], vec![2]]);
}

#[test]
fn no_cap_is_one_slice_and_no_rows_is_no_slices() {
    let (state, total) = plain();
    assert_eq!(plan(&state, "b1", &open(), total, None, None, now()).slices, vec![vec![0, 1, 2]]);
    let empty = ledger::fold(&ledger::read(&batch("b2", TREASURY, &[])));
    assert!(plan(&empty, "b2", &open(), Dec::ZERO, None, None, now()).slices.is_empty());
}

#[test]
fn a_single_row_over_the_slice_cap_is_its_own_slice_rather_than_stranded() {
    let state = state(&[batch("b1", TREASURY, &["100", "5"])]);
    let config = Config { max_batch_total_usdc: Some("50".to_owned()), ..open() };
    assert_eq!(plan(&state, "b1", &config, usdc("105"), None, None, now()).slices, vec![vec![0], vec![1]]);
}

#[test]
fn a_treasury_that_cannot_cover_a_slice_says_so_before_anything_moves() {
    let slice = usdc("60");
    let thin = Treasury { spot_usdc: usdc("59.999999"), ..Treasury::default() };
    let problems = covers(thin, slice, &open());
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("does not cover"));
    let exact = Treasury { spot_usdc: slice, ..Treasury::default() };
    assert!(covers(exact, slice, &open()).is_empty(), "exactly enough is enough");
}

#[test]
fn the_slice_check_applies_the_hype_floor_too() {
    let config = Config { min_hype_balance: Some("1".to_owned()), ..open() };
    let held = Treasury { spot_usdc: usdc("1000"), spot_hype: usdc("0.5"), ..Treasury::default() };
    let problems = covers(held, usdc("10"), &config);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("HYPE"));
}

#[test]
fn a_batch_that_is_not_in_the_ledger_plans_nothing_rather_than_panicking() {
    let (state, _) = plain();
    let plan = plan(&state, "ghost", &open(), Dec::ZERO, None, None, now());
    assert!(plan.payable.is_empty());
    assert!(plan.sendable.is_empty());
    assert!(plan.slices.is_empty());
    assert_eq!(plan.outstanding, Dec::ZERO);
}

#[test]
fn the_treasury_address_is_not_read_by_the_plan_itself() {
    let (state, total) = plain();
    let plan = plan(&state, "b1", &open(), total, None, None, now());
    assert_eq!(state.batch("b1").unwrap().treasury, at(TREASURY));
    assert_eq!(plan.batch_id, "b1");
}

#[test]
fn nothing_is_ready_to_send_until_a_plan_clears_and_has_rows() {
    let (state, total) = plain();
    assert!(plan(&state, "b1", &open(), total, None, None, now()).is_ready());
    assert!(!plan(&state, "ghost", &open(), Dec::ZERO, None, None, now()).is_ready(), "nothing to send");
    let capped = Config { max_csv_total_usdc: Some("50".to_owned()), ..open() };
    assert!(!plan(&state, "b1", &capped, total, None, None, now()).is_ready(), "refused");
}
