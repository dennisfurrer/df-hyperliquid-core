use super::*;
use crate::account::fixtures::read;

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
fn dex_abstraction_is_a_mode_of_its_own_and_keeps_ledgers_apart() {
    let mode: Abstraction = read("\"dexAbstraction\"");
    assert_eq!(mode, Abstraction::DexAbstraction);
    assert!(!mode.is_pooled());
    assert!(mode.is_standard());
    assert_eq!(mode.wire(), "dexAbstraction");
    for standard in [Abstraction::Default, Abstraction::Disabled, Abstraction::Unknown("new".to_owned())] {
        assert!(standard.is_standard(), "{standard}");
    }
    for pooled in [Abstraction::UnifiedAccount, Abstraction::PortfolioMargin] {
        assert!(!pooled.is_standard(), "{pooled}");
    }
}
