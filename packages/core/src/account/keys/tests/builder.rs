use super::*;
use crate::account::fixtures::{BUILDER, BUILDER_PERP, BUILDER_PERP_TESTNET, Recorded, address, block_on, dec, read};
use crate::account::holdings::perps::MarginSummary;

fn worth(value: &str) -> ClearinghouseState {
    ClearinghouseState {
        margin_summary: MarginSummary { account_value: dec(value), ..MarginSummary::default() },
        ..ClearinghouseState::default()
    }
}

#[test]
fn a_builder_with_a_hundred_usdc_and_standard_abstraction_is_ready() {
    assert_eq!(readiness(&worth("100.0"), &Abstraction::Disabled), Readiness::Ready);
    assert_eq!(readiness(&worth("3567.123456"), &Abstraction::Disabled), Readiness::Ready);
}

#[test]
fn under_a_hundred_usdc_is_not_ready_by_value() {
    let Readiness::NotReady(why) = readiness(&worth("99.999999"), &Abstraction::Disabled) else {
        panic!("99.999999 is under 100");
    };
    assert_eq!(why, [NotReady::TooLittle { value: dec("99.999999") }]);
    assert_eq!(why[0].to_string(), "99.999999 USDC perps account value, under the 100 a builder needs");
    for value in ["0.0", "-5"] {
        assert!(!readiness(&worth(value), &Abstraction::Disabled).is_ready(), "{value}");
    }
}

/// Standard is the mode `userAbstraction` answers as `disabled`; `default` is a mode never set,
/// and a builder is held to the one set.
#[test]
fn any_abstraction_but_standard_is_not_ready() {
    for mode in ["unifiedAccount", "portfolioMargin", "default", "dexAbstraction"] {
        let mode = Abstraction::parse(mode);
        let Readiness::NotReady(why) = readiness(&worth("5000"), &mode) else {
            panic!("{mode} is not standard");
        };
        assert_eq!(why[0].to_string(), format!("its account abstraction is {mode}, and a builder needs standard"));
    }
}

#[test]
fn every_reason_is_given_at_once() {
    assert_eq!(
        readiness(&worth("5"), &Abstraction::UnifiedAccount),
        Readiness::NotReady(vec![
            NotReady::TooLittle { value: dec("5") },
            NotReady::NotStandard(Abstraction::UnifiedAccount)
        ])
    );
}

/// The builder, live on 2026-10-08: standard on both networks, with next to nothing in perps on
/// mainnet and 3567.52 on testnet.
#[test]
fn the_live_builder_is_ready_on_testnet_and_not_on_mainnet() {
    let mainnet: ClearinghouseState = read(BUILDER_PERP);
    assert_eq!(
        readiness(&mainnet, &Abstraction::Disabled),
        Readiness::NotReady(vec![NotReady::TooLittle { value: dec("0.003569") }])
    );
    let testnet: ClearinghouseState = read(BUILDER_PERP_TESTNET);
    assert_eq!(readiness(&testnet, &Abstraction::Disabled), Readiness::Ready);
}

#[test]
fn readiness_reads_the_builder_s_perps_and_its_mode() {
    let user = format!(r#""user":"{BUILDER}""#);
    let state = format!(r#"{{"type":"clearinghouseState",{user}}}"#);
    let mode = format!(r#"{{"type":"userAbstraction",{user}}}"#);
    let transport = Recorded::new(&[(&state, BUILDER_PERP_TESTNET), (&mode, "\"disabled\"")]);
    assert_eq!(block_on(builder_readiness(&transport, address(BUILDER))), Ok(Readiness::Ready));
    assert_eq!(transport.asked(), [state, mode]);
}
