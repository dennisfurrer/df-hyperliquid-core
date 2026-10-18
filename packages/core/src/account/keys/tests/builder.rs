//! Whether a builder can be paid, and the fee an account approves it, as posted. The builder's
//! perps are live on both networks, 2026-10-08; its readiness read runs in the mocked journeys.

use super::*;
use crate::account::fixtures::{BUILDER_PERP, BUILDER_PERP_TESTNET, address, dec, read};
use crate::account::holdings::perps::MarginSummary;

fn worth(value: &str) -> ClearinghouseState {
    ClearinghouseState {
        margin_summary: MarginSummary { account_value: dec(value), ..MarginSummary::default() },
        ..ClearinghouseState::default()
    }
}

/// A hundred USDC in perps and standard as set: `disabled`, not `default`, a mode never set. Every
/// reason is given at once.
#[test]
fn a_builder_is_ready_with_a_hundred_usdc_in_perps_and_standard_set() {
    let poor = |value: &str| NotReady::TooLittle { value: dec(value) };
    let not = |mode: &str| NotReady::NotStandard(Abstraction::parse(mode));
    for (perp, mode, why) in [
        (worth("100.0"), "disabled", vec![]),
        (worth("3567.123456"), "disabled", vec![]),
        (read(BUILDER_PERP_TESTNET), "disabled", vec![]),
        (read(BUILDER_PERP), "disabled", vec![poor("0.003569")]),
        (worth("99.999999"), "disabled", vec![poor("99.999999")]),
        (worth("0.0"), "disabled", vec![poor("0")]),
        (worth("-5"), "disabled", vec![poor("-5")]),
        (worth("5000"), "default", vec![not("default")]),
        (worth("5000"), "unifiedAccount", vec![not("unifiedAccount")]),
        (worth("5000"), "portfolioMargin", vec![not("portfolioMargin")]),
        (worth("5000"), "dexAbstraction", vec![not("dexAbstraction")]),
        (worth("5"), "unifiedAccount", vec![poor("5"), not("unifiedAccount")]),
    ] {
        let expected = if why.is_empty() { Readiness::Ready } else { Readiness::NotReady(why) };
        let judged = readiness(&perp, &Abstraction::parse(mode));
        assert_eq!(judged.is_ready(), expected == Readiness::Ready);
        assert_eq!(judged, expected, "{mode}");
    }
    assert_eq!(poor("99.999999").to_string(), "99.999999 USDC perps account value, under the 100 a builder needs");
    assert_eq!(not("default").to_string(), "its account abstraction is default, and a builder needs standard");
}

/// `maxFeeRate` is the percent string the venue reads, signed as written, and back in tenths.
#[test]
fn a_builder_fee_is_approved_as_the_percent_the_venue_reads() {
    let builder = address("0x6967d7acd1ec5f210e9590666498e0cc5d13a843");
    for (tenths, rate) in [(100, "0.1%"), (880, "0.88%"), (88, "0.088%"), (1, "0.001%")] {
        let approve = ApproveBuilderFee::new(builder, BuilderFee::from_tenths(tenths), 1);
        assert_eq!((approve.max_fee_rate(), approve.max_fee()), (rate, Some(BuilderFee::from_tenths(tenths))));
        assert!(approve.wire(crate::client::Network::Mainnet).contains(&format!(r#""maxFeeRate":"{rate}""#)));
        assert_eq!(approve.builder(), builder);
    }
}
