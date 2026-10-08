//! hip4-tui's `engine/src/tests/payout/csv.rs`. The batch ids are what the other implementation
//! (the payout tool hip4-tui's ledger is shared with) computes for the same rows.

use hl_core::{Address, Dec};

use super::*;

const PLAIN: &str = "wallet,amount_usdc,memo\n\
0x6b9e773128f453f5c2c60935ee2de2cbc5390a24,12.5,june rewards\n\
0x1111111111111111111111111111111111111111,3.25,epoch 4\n";

fn sheet(text: &str) -> Sheet {
    read(text, false).unwrap()
}

#[test]
fn a_file_reads_into_rows_with_their_line_numbers() {
    let sheet = sheet(PLAIN);
    assert_eq!(sheet.rows.len(), 2);
    assert_eq!(sheet.rows[0].line, 2, "the header is line one");
    assert_eq!(sheet.rows[0].said(), "12.5");
    assert_eq!(sheet.rows[1].memo.as_deref(), Some("epoch 4"));
    assert_eq!(money::wire(sheet.total), "15.75");
}

#[test]
fn the_batch_id_is_what_the_other_implementation_computes() {
    let sheet = sheet(PLAIN);
    assert_eq!(
        sheet.fingerprint,
        "0x1111111111111111111111111111111111111111:3250000\n\
         0x6b9e773128f453f5c2c60935ee2de2cbc5390a24:12500000"
    );
    assert_eq!(sheet.legacy_id, "62f894d8f230c23e");
    let treasury: Address = "0x7661d1f77f4f0e2c9a6c0c1d9e8b7a6f5d4c3b2a".parse().unwrap();
    assert_eq!(sheet.batch_id(treasury), "62a968c99a2b4e4c");
}

#[test]
fn the_same_rows_in_another_order_are_the_same_batch() {
    let reversed = "wallet,amount_usdc\n\
0x1111111111111111111111111111111111111111,3.25\n\
0x6b9e773128f453f5c2c60935ee2de2cbc5390a24,12.5\n";
    assert_eq!(sheet(PLAIN).fingerprint, sheet(reversed).fingerprint);
}

#[test]
fn two_accounts_paying_one_list_are_two_batches() {
    let sheet = sheet(PLAIN);
    let first: Address = "0x0000000000000000000000000000000000000001".parse().unwrap();
    let second: Address = "0x0000000000000000000000000000000000000002".parse().unwrap();
    assert_ne!(sheet.batch_id(first), sheet.batch_id(second));
}

#[test]
fn every_spelling_of_the_columns_is_found() {
    for wallet in WALLET_HEADERS {
        for amount in AMOUNT_HEADERS {
            let text = format!("{wallet},{amount}\n0x1111111111111111111111111111111111111111,1\n");
            assert_eq!(sheet(&text).rows.len(), 1, "{wallet},{amount}");
        }
    }
}

#[test]
fn a_header_is_matched_however_it_is_spaced_or_cased() {
    let text = "\"Wallet Address\",\" Amount USDC \"\n0x1111111111111111111111111111111111111111,1\n";
    let sheet = sheet(text);
    assert_eq!(sheet.rows.len(), 1);
    assert_eq!(sheet.columns.wallet, "Wallet Address");
}

#[test]
fn a_file_with_no_recognisable_columns_says_what_it_looked_for() {
    let bad = read("who,how_much\n0x11,1\n", false).unwrap_err();
    assert!(matches!(bad, BadSheet::NoColumns { .. }));
    assert!(bad.to_string().contains("wallet"));
}

#[test]
fn a_zero_row_is_not_paid_and_is_not_an_error() {
    let text = "wallet,amount\n\
0x1111111111111111111111111111111111111111,0\n\
0x2222222222222222222222222222222222222222,1\n";
    let sheet = sheet(text);
    assert_eq!(sheet.rows.len(), 1);
    assert_eq!(sheet.zeroes.len(), 1);
    assert_eq!(sheet.zeroes[0].1, 2);
}

#[test]
fn an_address_that_is_not_an_address_refuses_the_whole_file() {
    let bad = read("wallet,amount\nnot-an-address,1\n", false).unwrap_err();
    assert!(matches!(bad, BadSheet::Unusable(_)));
    assert_eq!(bad.detail().len(), 1);
    assert!(bad.detail()[0].contains("line 2"));
}

#[test]
fn an_amount_that_is_not_an_amount_refuses_the_whole_file() {
    let bad = read("wallet,amount\n0x1111111111111111111111111111111111111111,-5\n", false).unwrap_err();
    assert!(matches!(bad, BadSheet::Unusable(_)));
}

#[test]
fn a_wallet_named_twice_is_refused_until_merging_is_asked_for() {
    let text = "wallet,amount\n\
0x1111111111111111111111111111111111111111,1\n\
0x1111111111111111111111111111111111111111,2\n";
    let bad = read(text, false).unwrap_err();
    assert!(matches!(bad, BadSheet::Duplicated(_)));
    assert!(bad.detail()[0].contains("lines 2, 3"));

    let merged = read(text, true).unwrap();
    assert_eq!(merged.rows.len(), 1);
    assert_eq!(merged.rows[0].said(), "3");
    assert_eq!(merged.merged.len(), 1);
    assert_eq!(merged.merged[0].lines, vec![2, 3]);
}

#[test]
fn merging_keeps_the_first_line_and_its_memo() {
    let text = "wallet,amount,memo\n\
0x2222222222222222222222222222222222222222,5,second\n\
0x1111111111111111111111111111111111111111,1,first\n\
0x1111111111111111111111111111111111111111,2,later\n";
    let merged = read(text, true).unwrap();
    let row = merged.rows.iter().find(|row| row.said() == "3").unwrap();
    assert_eq!(row.line, 3, "the first line it appeared on");
    assert_eq!(row.memo.as_deref(), Some("first"));
    assert_eq!(merged.rows[0].line, 2, "rows stay in file order");
}

#[test]
fn an_amount_is_re_rendered_rather_than_copied_from_the_file() {
    let text = "wallet,amount\n0x1111111111111111111111111111111111111111,1.50000000\n";
    assert_eq!(sheet(text).rows[0].said(), "1.5");
}

#[test]
fn quoted_fields_and_doubled_quotes_come_through() {
    let text = "wallet,amount,memo\n\
0x1111111111111111111111111111111111111111,1,\"a memo, with a comma\"\n\
0x2222222222222222222222222222222222222222,2,\"he said \"\"hello\"\"\"\n";
    let sheet = sheet(text);
    assert_eq!(sheet.rows[0].memo.as_deref(), Some("a memo, with a comma"));
    assert_eq!(sheet.rows[1].memo.as_deref(), Some("he said \"hello\""));
}

#[test]
fn either_line_ending_reads_the_same() {
    let unix = "wallet,amount\n0x1111111111111111111111111111111111111111,1\n";
    let windows = "wallet,amount\r\n0x1111111111111111111111111111111111111111,1\r\n";
    assert_eq!(sheet(unix).fingerprint, sheet(windows).fingerprint);
}

#[test]
fn a_blank_line_is_not_a_row() {
    let text = "wallet,amount\n\n0x1111111111111111111111111111111111111111,1\n\n";
    assert_eq!(sheet(text).rows.len(), 1);
}

#[test]
fn an_empty_file_and_one_with_nothing_payable_are_told_apart() {
    assert!(matches!(read("", false), Err(BadSheet::Empty)));
    let zeroes = "wallet,amount\n0x1111111111111111111111111111111111111111,0\n";
    assert!(matches!(read(zeroes, false), Err(BadSheet::Nothing)));
}

#[test]
fn a_file_with_no_memo_column_reads_without_one() {
    let sheet = sheet("wallet,amount\n0x1111111111111111111111111111111111111111,1\n");
    assert!(sheet.columns.memo.is_none());
    assert!(sheet.rows[0].memo.is_none());
}

#[test]
fn a_sheet_written_out_reads_back_to_the_same_batch() {
    let text = "wallet,amount,memo\n\
0x1111111111111111111111111111111111111111,1,\"a memo, with a comma\"\n\
0x2222222222222222222222222222222222222222,2.000001,\"he said \"\"hello\"\"\"\n\
0x3333333333333333333333333333333333333333,0.5,\n";
    let first = sheet(text);
    let written = write(&first.rows);
    assert!(written.starts_with("wallet,amount_usdc,memo\n"));
    let again = sheet(&written);
    assert_eq!(again.rows, first.rows, "every wallet, amount, line and memo");
    assert_eq!((again.fingerprint, again.legacy_id), (first.fingerprint, first.legacy_id));
}

#[test]
fn a_sheet_s_rows_are_the_lines_its_batch_records() {
    let sheet = sheet(PLAIN);
    let lines = sheet.lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        (lines[0].amount_usdc.as_str(), lines[0].line, lines[0].memo.as_deref()),
        ("12.5", 2, Some("june rewards"))
    );
    assert_eq!(lines[1].wallet, sheet.rows[1].wallet);
    assert_eq!(Dec::parse(&lines[1].amount_usdc).unwrap(), sheet.rows[1].amount);
}
