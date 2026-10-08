//! hip4-tui's `engine/src/tests/payout/config.rs`: the file a machine already has, read key for
//! key.

use super::*;

const LIVE: &str = r#"{
  "live": true,
  "allowFreshAccounts": true,
  "minPayoutUsdc": null,
  "maxSinglePayoutUsdc": "100000",
  "maxBatchTotalUsdc": "5000",
  "minHypeBalance": null,
  "maxCsvTotalUsdc": "20000",
  "sendPacingMs": 120,
  "maxAttempts": 5,
  "duplicateWindowDays": 7
}"#;

#[test]
fn the_file_this_machine_already_has_reads_key_for_key() {
    let config = Config::read(LIVE).unwrap();
    assert!(config.live);
    assert!(config.allow_fresh_accounts);
    assert_eq!(config.min_payout_usdc, None);
    assert_eq!(config.max_single_payout_usdc.as_deref(), Some("100000"));
    assert_eq!(config.send_pacing_ms, 120);
    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.duplicate_window_days, 7);
}

#[test]
fn it_writes_back_the_same_keys_it_read() {
    let config = Config::read(LIVE).unwrap();
    let first: serde_json::Value = serde_json::from_str(LIVE).unwrap();
    let second: serde_json::Value = serde_json::from_str(&config.write()).unwrap();
    assert_eq!(first, second);
    assert!(config.write().ends_with("}\n"), "a file ends its last line");
}

#[test]
fn a_file_with_nothing_in_it_is_the_cautious_default() {
    let config = Config::read("{}").unwrap();
    assert_eq!(config, Config::default());
    assert!(!config.live, "a send is a dry run until it is armed");
    assert!(!config.allow_fresh_accounts);
    assert_eq!(config.max_batch_total_usdc.as_deref(), Some("5000"));
    assert_eq!(config.max_csv_total_usdc.as_deref(), Some("20000"));
    assert_eq!(Config::read("").unwrap(), Config::default(), "a file not written yet is the default too");
    assert!(Config::read("{not json").is_err());
}

#[test]
fn a_missing_key_takes_its_default_rather_than_clearing_the_cap() {
    let config = Config::read(r#"{"live":true}"#).unwrap();
    assert!(config.live);
    assert_eq!(config.max_csv_total_usdc.as_deref(), Some("20000"));
    assert_eq!(config.max_attempts, 5);
}

#[test]
fn every_guard_has_a_key_and_says_something() {
    for guard in Guard::ALL {
        assert!(!guard.key().is_empty());
        assert!(!guard.about().is_empty());
        assert!(!Config::default().says(guard).is_empty(), "{}", guard.key());
    }
}

#[test]
fn the_guard_keys_are_the_ones_the_file_uses() {
    let written: serde_json::Value = serde_json::from_str(&Config::default().write()).unwrap();
    let object = written.as_object().unwrap();
    for guard in Guard::ALL {
        assert!(object.contains_key(guard.key()), "{}", guard.key());
    }
    assert_eq!(object.len(), Guard::ALL.len(), "and nothing else");
}

#[test]
fn a_cap_that_will_not_read_is_a_problem_rather_than_no_cap() {
    let config = Config { max_csv_total_usdc: Some("twenty thousand".to_owned()), ..Config::default() };
    assert_eq!(config.problems().len(), 1);
    assert!(config.problems()[0].contains("maxCsvTotalUsdc"));
    assert_eq!(config.limit(Guard::MaxCsvTotal), None);
}

#[test]
fn a_config_that_reads_has_no_problems() {
    let config = Config::read(LIVE).unwrap();
    assert!(config.problems().is_empty());
    assert_eq!(config.limit(Guard::MaxSinglePayout), Some(money::parse("100000").unwrap()));
    assert_eq!(config.limit(Guard::MinPayout), None, "unset is no cap");
}

#[test]
fn no_attempts_at_all_is_a_problem() {
    let config = Config { max_attempts: 0, ..Config::default() };
    assert!(config.problems().iter().any(|one| one.contains("maxAttempts")));
}

#[test]
fn every_way_of_clearing_a_limit_clears_it() {
    for said in ["", "  ", "none", "NONE", "null", "off", "-"] {
        assert_eq!(Config::read_limit(said).unwrap(), None, "{said}");
    }
}

#[test]
fn a_typed_limit_is_kept_as_typed_once_it_reads() {
    assert_eq!(Config::read_limit(" 5000 ").unwrap(), Some("5000".to_owned()));
    assert!(Config::read_limit("lots").is_err());
    assert!(Config::read_limit("-5").is_err());
}
