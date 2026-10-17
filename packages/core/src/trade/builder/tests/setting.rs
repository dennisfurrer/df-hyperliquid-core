//! Builder rules: a share of each market's cap in integer math, and the approval check.

use crate::trade::builder::fee::BuilderFee;
use crate::utils::{Address, Asset, AssetKind, Dec};

use super::*;
use crate::trade::builder::Builder;
use crate::trade::orders::{Order, PlaceOrder, Side, Tif};

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn terminal() -> Address {
    address("0x6967d7AcD1EC5f210E9590666498e0cC5D13a843")
}

fn tenths(fee: u64) -> BuilderFee {
    BuilderFee::from_tenths(fee)
}

fn at_88() -> BuilderSetting {
    BuilderSetting::share_of_cap(terminal(), 88).unwrap()
}

fn paying(asset: u32, fee: u64) -> PlaceOrder {
    let order = Order::limit(Asset::from_id(asset), Side::Buy, Dec::from_int(100), Dec::ONE, Tif::Gtc);
    PlaceOrder::one(order).through(Builder::new(terminal(), tenths(fee)))
}

#[test]
fn the_share_of_each_market_s_cap_is_integer_math() {
    let fees: Vec<(AssetKind, u64)> =
        AssetKind::ALL.into_iter().map(|kind| (kind, at_88().fee_on(kind).tenths())).collect();
    assert_eq!(
        fees,
        [(AssetKind::Perp, 88), (AssetKind::Spot, 880), (AssetKind::Hip3Perp, 88), (AssetKind::Outcome, 880)]
    );
    for kind in AssetKind::ALL {
        assert_eq!(at_88().on(kind).map(|builder| builder.fee()), Ok(at_88().fee_on(kind)), "{kind:?}");
    }
    let odd = BuilderSetting::share_of_cap(terminal(), 33).unwrap();
    assert_eq!((odd.fee_on(AssetKind::Perp), odd.fee_on(AssetKind::Spot)), (tenths(33), tenths(330)));
    let third = BuilderSetting::share_of_cap(terminal(), 1).unwrap();
    assert_eq!(third.fee_on(AssetKind::Perp), tenths(1), "a share is cut down, never up");
    assert!(BuilderSetting::share_of_cap(terminal(), 101).is_none(), "more than the whole cap is refused");
    assert_eq!(BuilderSetting::share_of_cap(terminal(), 100).unwrap().fee_on(AssetKind::Spot), tenths(1000));
}

#[test]
fn a_flat_fee_over_a_market_s_cap_is_refused_and_never_lowered() {
    let flat = BuilderSetting::flat(terminal(), tenths(500));
    assert_eq!(flat.on(AssetKind::Spot).map(|builder| builder.fee()), Ok(tenths(500)));
    assert_eq!(
        flat.on(AssetKind::Perp).unwrap_err().to_string(),
        "the builder fee of 500 is over the 100 a perp order allows (0.1%)"
    );
    let assets = [Asset::from_id(10_107), Asset::from_id(0)];
    assert!(flat.on_assets(assets).is_err(), "one perp order holds the action to the perp cap");
    assert_eq!(flat.on_assets([]), Ok(None), "no orders, no builder");
    assert_eq!(at_88().on_assets(assets).map(|builder| builder.map(|one| one.fee())), Ok(Some(tenths(88))));
}

/// What one approval has to cover: the most an order on any market pays it, 880 with each
/// market's own fee.
#[test]
fn the_approval_covers_the_most_any_market_pays() {
    assert_eq!(at_88().most(), tenths(880));
    assert_eq!(BuilderSetting::flat(terminal(), tenths(50)).most(), tenths(50));
    assert_eq!(BuilderSetting::flat(terminal(), tenths(500)).most(), tenths(500), "over the perp cap, still spot's");
    assert_eq!(BuilderSetting::flat(terminal(), tenths(1500)).most(), BuilderFee::ZERO, "over every cap: nothing goes");
}

#[test]
fn an_order_over_what_the_account_approved_asks_for_an_approval_first() {
    let setting = at_88();
    let perp = paying(0, 88);
    let spot = paying(10_107, 880);
    assert_eq!(setting.approval(&perp, &Approved::Unread, SignedBy::Account), Err(ApprovalRefused::NotRead));
    assert_eq!(
        setting.approval(&perp, &Approved::Upto(BuilderFee::ZERO), SignedBy::Account),
        Ok(Approval::ApproveFirst { approved: BuilderFee::ZERO, rate: tenths(880) })
    );
    assert_eq!(
        setting.approval(&perp, &Approved::Upto(tenths(88)), SignedBy::Account),
        Ok(Approval::Goes),
        "88 covers a perp"
    );
    assert_eq!(
        setting.approval(&spot, &Approved::Upto(tenths(88)), SignedBy::Account),
        Ok(Approval::ApproveFirst { approved: tenths(88), rate: tenths(880) }),
        "and not spot"
    );
    assert_eq!(setting.approval(&spot, &Approved::Upto(tenths(880)), SignedBy::Account), Ok(Approval::Goes));
}

#[test]
fn the_approval_is_the_exact_approve_builder_fee() {
    let first = at_88().approval(&paying(10_107, 880), &Approved::Upto(tenths(88)), SignedBy::Account).unwrap();
    assert_eq!(first.max_fee_rate().as_deref(), Some("0.88%"), "maxFeeRate as approveBuilderFee takes it");
    assert_eq!(Approval::Goes.max_fee_rate(), None);
}

#[test]
fn nothing_to_approve_needs_no_read() {
    let setting = at_88();
    let unrouted = PlaceOrder::one(Order::limit(Asset::from_id(0), Side::Buy, Dec::ONE, Dec::ONE, Tif::Gtc));
    let failed = Approved::Failed("timed out".to_owned());
    assert_eq!(setting.approval(&unrouted, &failed, SignedBy::Agent), Ok(Approval::Goes), "no builder");
    assert_eq!(setting.approval(&paying(0, 0), &failed, SignedBy::Agent), Ok(Approval::Goes), "a fee of 0");
}

#[test]
fn what_is_not_known_is_refused_rather_than_guessed() {
    let setting = at_88();
    let failed = Approved::Failed("https://api.hyperliquid.xyz timed out".to_owned());
    assert_eq!(
        setting.approval(&paying(0, 88), &failed, SignedBy::Account).unwrap_err().to_string(),
        "the builder fee this account has approved would not read, so it is not guessed at: https://api.hyperliquid.xyz timed out"
    );
    assert_eq!(
        setting.approval(&paying(0, 88), &Approved::Unread, SignedBy::Account).unwrap_err().to_string(),
        "the builder fee this account has approved is not read yet; try again in a moment"
    );
}

/// Only the main key signs `approveBuilderFee`; with an agent the order is refused, and the reason
/// names what the main key must approve.
#[test]
fn an_agent_cannot_approve_so_the_order_is_refused() {
    let refused = at_88().approval(&paying(10_107, 880), &Approved::Upto(tenths(88)), SignedBy::Agent).unwrap_err();
    assert_eq!(refused, ApprovalRefused::AgentCannotApprove { approved: tenths(88), rate: tenths(880) });
    assert_eq!(
        refused.to_string(),
        "the account's main key has to approve the builder fee up to 0.88% once (approved now: 0.088%); an agent key cannot sign approveBuilderFee"
    );
    assert_eq!(
        at_88().approval(&paying(0, 88), &Approved::Upto(tenths(88)), SignedBy::Agent),
        Ok(Approval::Goes),
        "already approved: an agent sends it"
    );
}

#[test]
fn an_order_naming_another_builder_is_not_checked_against_this_one() {
    let other = PlaceOrder::one(Order::limit(Asset::from_id(0), Side::Buy, Dec::ONE, Dec::ONE, Tif::Gtc))
        .through(Builder::new(address("0x1111111111111111111111111111111111111111"), tenths(10)));
    assert!(matches!(
        at_88().approval(&other, &Approved::Upto(tenths(880)), SignedBy::Account),
        Err(ApprovalRefused::OtherBuilder { .. })
    ));
}

#[test]
fn a_fee_reads_in_words() {
    assert_eq!(Approved::Unread.words(), "not read yet");
    assert_eq!(Approved::Upto(BuilderFee::ZERO).words(), "nothing yet");
    assert_eq!(Approved::Upto(tenths(88)).words(), "up to 0.088%");
    assert_eq!(Approved::Failed("timed out".to_owned()).words(), "would not read: timed out");
}
