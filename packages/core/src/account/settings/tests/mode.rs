//! An account's mode as `userAbstraction` answers it, and the modes it can be set to, spelled both
//! ways. Setting one runs in the mocked journeys.

use super::*;

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
        assert_eq!((read.is_pooled(), read.is_standard()), (pooled, !pooled), "{said}");
        assert_eq!(format!("\"{read}\""), said);
    }
    assert!(serde_json::from_str::<Abstraction>("null").is_err());
}

/// `userSetAbstraction` spells a mode out, `agentSetAbstraction` in one letter; `default`, a mode
/// never set, is none an account can be set to. Unified back to standard is refused.
#[test]
fn a_mode_set_is_spelled_both_ways_and_unified_never_goes_back_to_standard() {
    let spelled: Vec<(&str, &str)> = AbstractionMode::ALL.iter().map(|mode| (mode.wire(), mode.code())).collect();
    assert_eq!(spelled, [("disabled", "i"), ("unifiedAccount", "u"), ("portfolioMargin", "p")]);
    for mode in AbstractionMode::ALL {
        assert_eq!(Abstraction::parse(mode.wire()).wire(), mode.wire(), "a mode set reads back as itself");
        assert_eq!(
            (AbstractionMode::from_wire(mode.wire()), AbstractionMode::from_code(mode.code())),
            (Some(mode), Some(mode))
        );
    }
    assert_eq!((AbstractionMode::from_wire("default"), AbstractionMode::from_code("x")), (None, None));
    for (to, from, refused) in [
        (AbstractionMode::Disabled, Abstraction::UnifiedAccount, true),
        (AbstractionMode::UnifiedAccount, Abstraction::Disabled, false),
        (AbstractionMode::Disabled, Abstraction::Default, false),
        (AbstractionMode::PortfolioMargin, Abstraction::UnifiedAccount, false),
    ] {
        assert_eq!(to.is_refused_from(&from), refused, "{from} to {}", to.wire());
    }
}
