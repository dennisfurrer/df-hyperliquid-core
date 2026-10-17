use super::*;

/// The modes `userAbstraction` answers, and none read: only unified and portfolio margin pool, and
/// a mode not read or not known stays unknown, so a spot token leaves by `sendAsset`.
#[test]
fn only_unified_and_portfolio_margin_pool_spot() {
    let of = |word: &str| Keeping::of(Some(&Abstraction::parse(word)), None);
    assert_eq!(of("unifiedAccount"), Keeping::Unified);
    assert_eq!(of("portfolioMargin"), Keeping::Portfolio);
    assert_eq!(of("dexAbstraction"), Keeping::Native);
    for apart in ["disabled", "default"] {
        assert_eq!(of(apart), Keeping::Apart, "{apart}");
    }
    for unknown in ["", "somethingNew"] {
        assert_eq!(of(unknown), Keeping::Unknown, "{unknown}");
    }
    assert_eq!(Keeping::of(None, None), Keeping::Unknown, "not read yet");
    assert!(Keeping::Unified.pools() && Keeping::Portfolio.pools());
    assert!(!Keeping::Apart.pools() && !Keeping::Native.pools() && !Keeping::Unknown.pools());
}
