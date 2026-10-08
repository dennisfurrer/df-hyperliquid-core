//! hip4-tui's `engine/src/tests/payout/ledger.rs` and `tui/src/tests/payouts.rs`, and its
//! `net/src/tests/watch.rs` on the transfers a reconcile reads, which carries live mainnet rows of
//! `userNonFundingLedgerUpdates` in the shapes the venue sends. What a reconcile writes follows
//! hip4-tui's `reconcile` in `tui/src/main.rs`.

use hl_core::Address;

use super::*;
use crate::hip4::LedgerRow;

const ONE: &str = "0x1111111111111111111111111111111111111111";
const TWO: &str = "0x2222222222222222222222222222222222222222";
const TREASURY: &str = "0x7661d1f77f4f0e2c9a6c0c1d9e8b7a6f5d4c3b2a";
const OTHER: &str = "0x9999999999999999999999999999999999999999";

fn at(hex: &str) -> Address {
    hex.parse().unwrap()
}

fn when(said: &str) -> At {
    At::parse(said).unwrap()
}

fn created(batch: &str, treasury: &str) -> String {
    format!(
        r#"{{"type":"batch_created","at":"2026-09-01T10:00:00.000Z","batch_id":"{batch}",
        "source":"payouts.csv","treasury":"{treasury}","account_label":"[JB] Marketing",
        "rows":[{{"wallet":"{ONE}","amount_usdc":"12.5","line":2,"memo":"june"}},
                {{"wallet":"{TWO}","amount_usdc":"3.25","line":3}}]}}"#
    )
    .replace('\n', "")
}

fn claimed(batch: &str, wallet: &str, at: &str) -> String {
    format!(r#"{{"type":"row_claimed","at":"{at}","batch_id":"{batch}","wallet":"{wallet}","run_id":"r1"}}"#)
}

fn sent(batch: &str, wallet: &str, at: &str) -> String {
    format!(r#"{{"type":"row_sent","at":"{at}","batch_id":"{batch}","wallet":"{wallet}","run_id":"r1"}}"#)
}

fn log(lines: &[String]) -> State {
    fold(&read(&lines.join("\n")))
}

#[test]
fn a_batch_starts_with_every_row_pending() {
    let state = log(&[created("b1", TREASURY)]);
    let batch = state.batch("b1").unwrap();
    assert_eq!(batch.row_count, 2);
    assert_eq!(money::wire(batch.total), "15.75");
    assert_eq!(batch.account_label.as_deref(), Some("[JB] Marketing"));
    assert_eq!(batch.treasury, at(TREASURY));
    assert!(state.rows("b1").iter().all(|row| row.status == Status::Pending));
    assert_eq!(state.rows("b1")[0].memo.as_deref(), Some("june"));
}

#[test]
fn a_claim_then_a_send_walks_one_row_to_sent() {
    let state = log(&[
        created("b1", TREASURY),
        claimed("b1", ONE, "2026-09-01T10:01:00.000Z"),
        sent("b1", ONE, "2026-09-01T10:01:01.000Z"),
        format!(
            r#"{{"type":"row_hash","at":"2026-09-01T10:02:00.000Z","batch_id":"b1","wallet":"{ONE}","tx_hash":"0xabc"}}"#
        ),
    ]);
    let row = &state.rows("b1")[0];
    assert_eq!(row.status, Status::Sent);
    assert_eq!(row.attempts, 1);
    assert_eq!(row.tx_hash.as_deref(), Some("0xabc"));
    assert_eq!(row.sent_by.as_deref(), Some("r1"));
    assert_eq!(row.sent_at, Some(when("2026-09-01T10:01:01Z")));
}

#[test]
fn a_claim_with_no_answer_leaves_the_row_submitting_and_unpayable() {
    let state = log(&[created("b1", TREASURY), claimed("b1", ONE, "2026-09-01T10:01:00.000Z")]);
    assert_eq!(state.rows("b1")[0].status, Status::Submitting);
    let payable: Vec<Address> = state.payable("b1", 5).iter().map(|row| row.wallet).collect();
    assert_eq!(payable, vec![at(TWO)]);
}

#[test]
fn a_failed_row_is_payable_until_it_runs_out_of_attempts() {
    let mut lines = vec![created("b1", TREASURY)];
    for turn in 0..3 {
        lines.push(claimed("b1", ONE, &format!("2026-09-01T10:0{turn}:00.000Z")));
        lines.push(format!(
            r#"{{"type":"row_failed","at":"2026-09-01T10:0{turn}:01.000Z","batch_id":"b1","wallet":"{ONE}","run_id":"r1","error":"refused"}}"#
        ));
    }
    let state = log(&lines);
    let row = &state.rows("b1")[0];
    assert_eq!(row.attempts, 3);
    assert_eq!(row.last_error.as_deref(), Some("refused"));
    assert_eq!(state.payable("b1", 5).len(), 2, "still under the ceiling");
    assert_eq!(state.payable("b1", 3).len(), 1, "at it, so only the other row");
}

#[test]
fn a_row_set_aside_by_hand_is_no_longer_owed() {
    let state = log(&[
        created("b1", TREASURY),
        format!(
            r#"{{"type":"row_status","at":"2026-09-01T11:00:00.000Z","batch_id":"b1","wallet":"{ONE}","status":"dismissed","note":"settled by wire"}}"#
        ),
    ]);
    let row = &state.rows("b1")[0];
    assert_eq!(row.status, Status::Dismissed);
    assert_eq!(row.last_error.as_deref(), Some("settled by wire"));
    assert!(!row.status.is_outstanding());
    assert_eq!(state.payable("b1", 5).len(), 1);
}

#[test]
fn re_importing_a_batch_does_not_reset_what_it_has_already_paid() {
    let state = log(&[created("b1", TREASURY), sent("b1", ONE, "2026-09-01T10:01:00.000Z"), created("b1", TREASURY)]);
    assert_eq!(state.batches.len(), 1, "the first import stands");
    assert_eq!(state.rows("b1")[0].status, Status::Sent);
}

#[test]
fn an_event_for_a_batch_that_was_never_created_is_ignored() {
    let state = log(&[sent("ghost", ONE, "2026-09-01T10:01:00.000Z")]);
    assert!(state.batches.is_empty());
    assert!(state.rows("ghost").is_empty());
}

#[test]
fn a_torn_last_line_is_dropped_and_everything_before_it_stands() {
    let text = format!("{}\n{{\"type\":\"row_se", created("b1", TREASURY));
    let state = fold(&read(&text));
    assert_eq!(state.batches.len(), 1);
    assert_eq!(state.rows("b1").len(), 2);
}

#[test]
fn a_run_carries_the_instant_it_started_as_well_as_the_one_it_ended() {
    let state = log(&[
        created("b1", TREASURY),
        r#"{"type":"run_started","at":"2026-09-01T10:00:00.000Z","run_id":"r1","batch_id":"b1","mode":"live"}"#.to_owned(),
        r#"{"type":"run_finished","at":"2026-09-01T10:05:00.000Z","run_id":"r1","batch_id":"b1","sent":2,"failed":0,"skipped":0,"quarantined":0,"sent_usdc":"15.75"}"#.to_owned(),
    ]);
    let run = state.runs_on("b1")[0];
    assert_eq!(run.started_at, Some(when("2026-09-01T10:00:00Z")));
    assert_eq!(run.at, when("2026-09-01T10:05:00Z"));
    assert_eq!(run.sent, 2);
    assert!(run.stopped_reason.is_none());
}

#[test]
fn a_legacy_batch_resumes_only_for_the_account_that_paid_it() {
    let state = log(&[created("legacy", TREASURY)]);
    assert_eq!(state.resolve("scoped", "legacy", at(TREASURY)), "legacy");
    assert_eq!(state.resolve("scoped", "legacy", at(OTHER)), "scoped");
}

#[test]
fn a_batch_already_recorded_under_its_scoped_id_wins_over_the_legacy_one() {
    let state = log(&[created("legacy", TREASURY), created("scoped", TREASURY)]);
    assert_eq!(state.resolve("scoped", "legacy", at(TREASURY)), "scoped");
}

#[test]
fn a_wallet_paid_the_same_amount_lately_is_found_across_batches() {
    let state = log(&[created("b1", TREASURY), sent("b1", ONE, "2026-09-01T10:01:00.000Z")]);
    let amount = money::parse("12.5").unwrap();
    let now = when("2026-09-03T10:00:00Z");
    assert_eq!(state.recently_paid(at(ONE), amount, 7, "b2", now).len(), 1, "paid two days ago, inside the window");
    assert!(state.recently_paid(at(ONE), amount, 1, "b2", now).is_empty(), "outside the window");
    assert!(state.recently_paid(at(ONE), amount, 7, "b1", now).is_empty(), "its own batch is never a duplicate");
    let different = money::parse("12.6").unwrap();
    assert!(state.recently_paid(at(ONE), different, 7, "b2", now).is_empty(), "a different amount is not a repeat");
}

#[test]
fn a_duplicate_is_found_even_when_another_account_paid_it() {
    let state = log(&[created("b1", OTHER), sent("b1", ONE, "2026-09-01T10:01:00.000Z")]);
    let hits = state.recently_paid(at(ONE), money::parse("12.5").unwrap(), 7, "b2", when("2026-09-02T10:00:00Z"));
    assert_eq!(hits.len(), 1, "the mix-up separate budgets exist to catch");
}

#[test]
fn only_a_sent_row_counts_as_already_paid() {
    let state = log(&[created("b1", TREASURY), claimed("b1", ONE, "2026-09-01T10:01:00.000Z")]);
    let hits = state.recently_paid(at(ONE), money::parse("12.5").unwrap(), 7, "b2", when("2026-09-02T10:00:00Z"));
    assert!(hits.is_empty(), "a claim is not a payment");
}

#[test]
fn the_totals_are_kept_per_account() {
    let state = log(&[created("b1", TREASURY), sent("b1", ONE, "2026-09-01T10:01:00.000Z"), created("b2", OTHER)]);
    let totals = state.totals();
    let mine = &totals[&at(TREASURY)];
    assert_eq!(mine.batches, 1);
    assert_eq!(mine.rows_sent, 1);
    assert_eq!(money::wire(mine.sent), "12.5");
    assert_eq!(mine.outstanding, 1);
    assert_eq!(mine.last_payout_at, Some(when("2026-09-01T10:01:00Z")));
    let theirs = &totals[&at(OTHER)];
    assert_eq!(theirs.rows_sent, 0);
    assert_eq!(theirs.outstanding, 2);
}

#[test]
fn the_tally_names_every_status_even_the_empty_ones() {
    let state = log(&[created("b1", TREASURY)]);
    let counts = tally(state.rows("b1"));
    assert_eq!(counts.len(), Status::ALL.len());
    assert_eq!(counts["pending"], 2);
    assert_eq!(counts["sent"], 0);
}

#[test]
fn an_event_round_trips_through_the_json_the_other_tool_writes() {
    let text = format!(
        r#"{{"type":"row_sent","at":"2026-09-01T10:01:01.000Z","batch_id":"b1","wallet":"{ONE}","run_id":"r1","tx_hash":"0xabc"}}"#
    );
    let event: Event = serde_json::from_str(&text).unwrap();
    let first: serde_json::Value = serde_json::from_str(&text).unwrap();
    let second: serde_json::Value = serde_json::from_str(&line(&event).unwrap()).unwrap();
    assert_eq!(first, second, "written back the same way it was read");
    assert!(line(&event).unwrap().ends_with('\n'), "one event a line");
}

#[test]
fn an_optional_field_is_left_out_rather_than_written_as_null() {
    let event = Event::RowSent {
        at: when("2026-09-01T10:01:01Z"),
        batch_id: "b1".to_owned(),
        wallet: at(ONE),
        run_id: "r1".to_owned(),
        tx_hash: None,
    };
    let written = line(&event).unwrap();
    assert!(!written.contains("tx_hash"), "{written}");
}

#[test]
fn the_batches_are_listed_newest_first() {
    let state = log(&[created("older", TREASURY), created("newer", TREASURY).replace("2026-09-01", "2026-09-08")]);
    let listed: Vec<&str> = state.newest().iter().map(|batch| batch.batch_id.as_str()).collect();
    assert_eq!(listed, ["newer", "older"]);
}

#[test]
fn what_is_still_owed_leaves_out_what_is_settled() {
    let paid = log(&[created("b1", TREASURY), sent("b1", ONE, "2026-09-01T10:01:00.000Z")]);
    assert_eq!(money::wire(outstanding(paid.rows("b1"))), "3.25");
    let pending = log(&[created("b1", TREASURY)]);
    assert_eq!(outstanding(pending.rows("b1")), money::parse("15.75").unwrap());
}

const UPDATES: &str = r#"[
 {"time":1788276890971,"hash":"0x37ce4b34",
  "delta":{"type":"spotTransfer","token":"USDC","amount":"15.0","usdcValue":"15.0",
    "user":"0x7661d1256b43acbafe005ad52712573a15a180f8",
    "destination":"0x623b28bd647ba19e4b8374a467cc24d09038cc9c",
    "fee":"1.0","nonce":1788276889863,"feeToken":"USDC"}},
 {"time":1788276355856,"hash":"0xc63620c6",
  "delta":{"type":"send","token":"USDC","amount":"2000.0","usdcValue":"2000.0",
    "user":"0xc283f0494f7c6070cd6af7860467cd02d8aa9ee4",
    "destination":"0x7661d1256b43acbafe005ad52712573a15a180f8",
    "sourceDex":"spot","destinationDex":"spot","fee":"1.0","feeToken":"USDC"}},
 {"time":1788276000000,"hash":"0xdeadbeef",
  "delta":{"type":"deposit","usdc":"100.0"}}]"#;

fn updates() -> Vec<LedgerRow> {
    serde_json::from_str(UPDATES).unwrap()
}

#[test]
fn only_the_usdc_that_left_this_account_is_a_payout() {
    let out = transfers(&updates(), at("0x7661d1256b43acbafe005ad52712573a15a180f8"));
    assert_eq!(out.len(), 1, "the incoming send and the deposit are not payouts");
    assert_eq!(out[0].hash, "0x37ce4b34");
    assert_eq!(out[0].amount, money::parse("15.0").unwrap());
    assert_eq!(out[0].destination, at("0x623b28bd647ba19e4b8374a467cc24d09038cc9c"));
    assert_eq!(out[0].time, 1_788_276_890_971);
}

#[test]
fn a_transfer_arriving_is_never_read_as_one_that_went_out() {
    let out = transfers(&updates(), at("0xc283f0494f7c6070cd6af7860467cd02d8aa9ee4"));
    assert_eq!(out.len(), 1, "for the other account it is the one that left");
    assert_eq!(out[0].hash, "0xc63620c6");
}

#[test]
fn a_transfer_of_something_other_than_usdc_is_not_a_payout() {
    let rows: Vec<LedgerRow> = serde_json::from_str(
        r#"[{"time":1,"hash":"0x1","delta":{"type":"spotTransfer","token":"HYPE",
        "amount":"1.0","user":"0x7661d1256b43acbafe005ad52712573a15a180f8",
        "destination":"0x623b28bd647ba19e4b8374a467cc24d09038cc9c"}}]"#,
    )
    .unwrap();
    assert!(transfers(&rows, at("0x7661d1256b43acbafe005ad52712573a15a180f8")).is_empty());
}

#[test]
fn a_reconcile_finds_each_claim_in_the_venue_s_ledger_or_sets_it_aside() {
    let lines = [
        created("b1", TREASURY),
        claimed("b1", ONE, "2026-09-01T10:01:00.000Z"),
        claimed("b1", TWO, "2026-09-01T10:02:00.000Z"),
    ];
    let state = log(&lines);
    assert_eq!(
        waiting_since(&state, "b1"),
        Some(when("2026-09-01T10:01:00Z")),
        "the ledger is read from the first claim"
    );
    let found = [
        Transfer { hash: "0xaaa".to_owned(), time: 1, destination: at(ONE), amount: money::parse("12.5").unwrap() },
        Transfer { hash: "0xbbb".to_owned(), time: 2, destination: at(TWO), amount: money::parse("3.2").unwrap() },
    ];
    let now = when("2026-09-01T11:00:00Z");
    let events = reconcile(&state, "b1", &found, now);
    assert_eq!(events.len(), 2, "one answer a claim");
    let mut all = read(&lines.join("\n"));
    all.extend(events);
    let after = fold(&all);
    let one = &after.rows("b1")[0];
    assert_eq!(
        (one.status, one.tx_hash.as_deref(), one.sent_by.as_deref()),
        (Status::Sent, Some("0xaaa"), Some("reconcile"))
    );
    assert_eq!(one.sent_at, Some(now));
    let two = &after.rows("b1")[1];
    assert_eq!(two.status, Status::NeedsReview, "3.2 went to it, not the 3.25 claimed, so a person looks");
    assert!(two.last_error.as_deref().is_some_and(|note| note.contains("3.25")));
    assert_eq!(waiting_since(&after, "b1"), None, "nothing waits on an answer now");
    assert!(reconcile(&after, "b1", &found, now).is_empty());
}
