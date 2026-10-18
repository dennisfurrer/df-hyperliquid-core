//! Builder rules: a share of each market's cap in integer math, a flat fee never lowered, and the
//! approval an order needs before it goes.

use crate::trade::builder::fee::BuilderFee;
use crate::utils::{Address, Asset, AssetKind, Dec};

use super::*;
use crate::account::Key;
use crate::trade::builder::Builder;
use crate::trade::orders::{Order, PlaceOrder, Side, Tif};

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn builder() -> Address {
    address("0x6967d7AcD1EC5f210E9590666498e0cC5D13a843")
}

fn tenths(fee: u64) -> BuilderFee {
    BuilderFee::from_tenths(fee)
}

fn at_88() -> BuilderSetting {
    BuilderSetting::share_of_cap(builder(), 88).unwrap()
}

fn paying(asset: u32, fee: u64) -> PlaceOrder {
    let order = Order::limit(Asset::from_id(asset), Side::Buy, Dec::from_int(100), Dec::ONE, Tif::Gtc);
    PlaceOrder::one(order).through(Builder::new(builder(), tenths(fee)))
}

#[test]
fn a_share_of_each_market_s_cap_is_cut_down_to_whole_tenths() {
    let share = |percent: u64| BuilderSetting::share_of_cap(builder(), percent).unwrap();
    for (percent, perp, spot) in [(88, 88, 880), (33, 33, 330), (1, 1, 10), (100, 100, 1000)] {
        let fees = AssetKind::ALL.map(|kind| share(percent).fee_on(kind).tenths());
        assert_eq!(fees, [perp, spot, perp, spot], "{percent}%: perp, spot, HIP-3, outcome");
        for kind in AssetKind::ALL {
            assert_eq!(share(percent).on(kind).map(|builder| builder.fee()), Ok(share(percent).fee_on(kind)));
        }
        assert_eq!(share(percent).most(), tenths(spot), "one approval covers the dearest market");
    }
    assert!(BuilderSetting::share_of_cap(builder(), 101).is_none(), "more than the whole cap is refused");
}

#[test]
fn a_flat_fee_over_a_market_s_cap_is_refused_and_never_lowered() {
    let flat = |fee: u64| BuilderSetting::flat(builder(), tenths(fee));
    assert_eq!(flat(500).on(AssetKind::Spot).map(|builder| builder.fee()), Ok(tenths(500)));
    assert_eq!(
        flat(500).on(AssetKind::Perp).unwrap_err().to_string(),
        "the builder fee of 500 is over the 100 a perp order allows (0.1%)"
    );
    let assets = [Asset::from_id(10_107), Asset::from_id(0)];
    assert!(flat(500).on_assets(assets).is_err(), "one perp order holds the action to the perp cap");
    assert_eq!(flat(500).on_assets([]), Ok(None), "no orders, no builder");
    assert_eq!(at_88().on_assets(assets).map(|builder| builder.map(|one| one.fee())), Ok(Some(tenths(88))));
    for (fee, most) in [(50, 50), (500, 500), (1500, 0)] {
        assert_eq!(flat(fee).most(), tenths(most), "{fee}: what goes on some market, or nothing");
    }
}

/// Only the main key signs `approveBuilderFee`, and what the account approved is never guessed.
#[test]
fn an_order_needs_an_approval_only_for_a_fee_above_what_the_account_approved() {
    let failed = || Approved::Failed("timed out".to_owned());
    let unrouted = PlaceOrder::one(Order::limit(Asset::from_id(0), Side::Buy, Dec::ONE, Dec::ONE, Tif::Gtc));
    let other =
        unrouted.clone().through(Builder::new(address("0x1111111111111111111111111111111111111111"), tenths(10)));
    let first = |approved: u64| Ok(Approval::ApproveFirst { approved: tenths(approved), rate: tenths(880) });
    let cases: [(PlaceOrder, Approved, Key, Result<Approval, ApprovalRefused>); 11] = [
        (unrouted, failed(), Key::Agent, Ok(Approval::Goes)),
        (paying(0, 0), failed(), Key::Agent, Ok(Approval::Goes)),
        (paying(0, 88), Approved::Unread, Key::Main, Err(ApprovalRefused::NotRead)),
        (paying(0, 88), failed(), Key::Main, Err(ApprovalRefused::Unreadable("timed out".to_owned()))),
        (paying(0, 88), Approved::Upto(BuilderFee::ZERO), Key::Main, first(0)),
        (paying(0, 88), Approved::Upto(tenths(88)), Key::Agent, Ok(Approval::Goes)),
        (paying(10_107, 880), Approved::Upto(tenths(88)), Key::Main, first(88)),
        (
            paying(10_107, 880),
            Approved::Upto(tenths(88)),
            Key::Agent,
            Err(ApprovalRefused::AgentCannotApprove { approved: tenths(88), rate: tenths(880) }),
        ),
        (paying(10_107, 880), Approved::Upto(tenths(880)), Key::Agent, Ok(Approval::Goes)),
        (
            other,
            Approved::Upto(tenths(880)),
            Key::Main,
            Err(ApprovalRefused::OtherBuilder {
                named: address("0x1111111111111111111111111111111111111111"),
                set: builder(),
            }),
        ),
        (
            paying(0, 1000),
            Approved::Upto(tenths(100)),
            Key::Main,
            Ok(Approval::ApproveFirst { approved: tenths(100), rate: tenths(1000) }),
        ),
    ];
    for (order, approved, key, expected) in cases {
        assert_eq!(at_88().approval(&order, &approved, key), expected, "{order:?} {approved:?} {key:?}");
    }
}

#[test]
fn an_approval_and_each_refusal_read_in_words() {
    let first = Approval::ApproveFirst { approved: tenths(88), rate: tenths(880) };
    assert_eq!((first.max_fee_rate().as_deref(), Approval::Goes.max_fee_rate()), (Some("0.88%"), None));
    for (refused, words) in [
        (ApprovalRefused::NotRead, "the builder fee this account has approved is not read yet; try again in a moment"),
        (
            ApprovalRefused::Unreadable("https://api.hyperliquid.xyz timed out".to_owned()),
            "the builder fee this account has approved would not read, so it is not guessed at: https://api.hyperliquid.xyz timed out",
        ),
        (
            ApprovalRefused::AgentCannotApprove { approved: tenths(88), rate: tenths(880) },
            "the account's main key has to approve the builder fee up to 0.88% once (approved now: 0.088%); an agent key cannot sign approveBuilderFee",
        ),
    ] {
        assert_eq!(refused.to_string(), words);
    }
    for (approved, words) in [
        (Approved::Unread, "not read yet"),
        (Approved::Upto(BuilderFee::ZERO), "nothing yet"),
        (Approved::Upto(tenths(88)), "up to 0.088%"),
        (Approved::Failed("timed out".to_owned()), "would not read: timed out"),
    ] {
        assert_eq!(approved.words(), words);
    }
}
