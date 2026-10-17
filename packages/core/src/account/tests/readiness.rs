use crate::client::{ApiError, TransportError};
use crate::trade::builder::fee::BuilderFee;

use super::*;
use crate::account::builder::ApproveBuilderFee;
use crate::account::fixtures::{BUILDER, BUILDER_PERP, BUILDER_PERP_TESTNET, Recorded, address, block_on, dec, read};
use crate::account::state::MarginSummary;

fn worth(value: &str) -> PerpState {
    PerpState {
        margin_summary: MarginSummary { account_value: dec(value), ..MarginSummary::default() },
        ..PerpState::default()
    }
}

fn tenths(fee: u64) -> BuilderFee {
    BuilderFee::from_tenths(fee)
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
    let mainnet: PerpState = read(BUILDER_PERP);
    assert_eq!(
        readiness(&mainnet, &Abstraction::Disabled),
        Readiness::NotReady(vec![NotReady::TooLittle { value: dec("0.003569") }])
    );
    let testnet: PerpState = read(BUILDER_PERP_TESTNET);
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

#[test]
fn an_order_over_what_the_account_approved_asks_for_an_approval_first() {
    assert_eq!(
        clearance(&Approved::Unread, Some(tenths(88))),
        Clearance::Refused(
            "the builder fee this account has approved is not read yet; try again in a moment".to_owned()
        ),
        "not read yet is not guessed at"
    );
    assert_eq!(
        clearance(&Approved::Upto(BuilderFee::ZERO), Some(tenths(88))),
        Clearance::Approve { approved: BuilderFee::ZERO }
    );
    let approved = Approved::Upto(tenths(88));
    assert_eq!(clearance(&approved, Some(tenths(88))), Clearance::Clear, "88 covers a perp");
    assert_eq!(clearance(&approved, Some(tenths(880))), Clearance::Approve { approved: tenths(88) }, "and not spot");
    assert_eq!(clearance(&Approved::Upto(tenths(880)), Some(tenths(880))), Clearance::Clear);
    assert_eq!(clearance(&approved, None), Clearance::Clear, "no builder");
    assert_eq!(clearance(&approved, Some(BuilderFee::ZERO)), Clearance::Clear, "a fee of 0 needs no approval");
    let failed = Approved::after(Err(ApiError::Transport(TransportError::Unreachable("no answer".to_owned()))));
    let Clearance::Refused(why) = clearance(&failed, Some(tenths(880))) else {
        panic!("an approval that would not read was guessed at");
    };
    assert!(why.starts_with("the builder fee this account has approved would not read, so it is not guessed at: "));
    assert_eq!(clearance(&failed, Some(BuilderFee::ZERO)), Clearance::Clear, "even then, a fee of 0 needs nothing");
}

#[test]
fn what_the_account_approved_reads_in_words() {
    assert_eq!(Approved::Unread.words(), "not read yet");
    assert_eq!(Approved::Upto(BuilderFee::ZERO).words(), "nothing yet");
    assert_eq!(Approved::Upto(tenths(88)).words(), "up to 0.088% (8.8 bps)");
    assert_eq!(Approved::Upto(tenths(880)).words(), "up to 0.88% (88 bps)");
    assert_eq!(Approved::Failed("timed out".to_owned()).words(), "would not read: timed out");
    assert_eq!(Approved::after(Ok(tenths(880))), Approved::Upto(tenths(880)));
}

/// An approval the venue took is what the account has approved from then
/// on, for the builder named and no other; a lower one lowers it.
#[test]
fn an_approval_taken_is_what_the_account_has_approved() {
    let builder = address(BUILDER);
    let other = address("0x1111111111111111111111111111111111111111");
    let approved = Approved::Upto(BuilderFee::ZERO).taken(builder, &ApproveBuilderFee::new(builder, tenths(880), 1));
    assert_eq!(approved, Approved::Upto(tenths(880)));
    let unchanged = approved.clone().taken(builder, &ApproveBuilderFee::new(other, tenths(10), 2));
    assert_eq!(unchanged, approved, "another builder's");
    let lower = approved.taken(builder, &ApproveBuilderFee::new(builder, tenths(100), 3));
    assert_eq!(lower, Approved::Upto(tenths(100)));
}
