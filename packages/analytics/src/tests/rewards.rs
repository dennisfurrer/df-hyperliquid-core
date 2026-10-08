//! hip4-tui's `net/src/tests/fees.rs` on `referral`, and hl-account's recorded answer for hip4-tui's
//! builder (`0x6967..a843`, mainnet, 2026-10-08), whose referral share is checked against the
//! figure the same answer reports for its one paying referral.

use hl_account::Referral;
use hl_core::Dec;

use super::*;

const LIVE: &str = include_str!("../../fixtures/referral_builder.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn read(text: &str) -> Rewards {
    Rewards::of(&serde_json::from_str::<Referral>(text).unwrap())
}

#[test]
fn a_referral_payload_is_read_past_the_builder_total() {
    let rewards = read(
        r#"{"referredBy": null, "cumVlm": "12283.6",
            "unclaimedRewards": "36.66020118", "claimedRewards": "0.0",
            "builderRewards": "36.66020118",
            "referrerState": {"stage": "needToCreateCode"},
            "rewardHistory": []}"#,
    );
    assert_eq!(rewards.builder, dec("36.66020118"));
    assert_eq!(rewards.unclaimed, rewards.builder, "none of it claimed");
    assert_eq!(rewards.referral(), Dec::ZERO, "the whole pot is the builder fee");
    assert_eq!(rewards.volume, dec("12283.6"));
    assert_eq!(rewards.stage, Stage::NeedsCode);
    assert!(!rewards.stage.earning());
}

#[test]
fn a_venue_that_has_not_traded_enough_cannot_earn_a_referral_fee() {
    let rewards = read(
        r#"{"cumVlm": "180.68", "unclaimedRewards": "0.0",
            "claimedRewards": "0.0", "builderRewards": "0.0",
            "referrerState": {"stage": "needToTrade", "data": {"required": "9819.32"}}}"#,
    );
    assert_eq!(rewards.stage, Stage::NeedsVolume { required: dec("9819.32") });
    assert_eq!(rewards.referral(), Dec::ZERO);
}

#[test]
fn the_bar_a_code_takes_is_what_was_traded_plus_what_is_short() {
    let short =
        Rewards { volume: dec("180.68"), stage: Stage::NeedsVolume { required: dec("9819.32") }, ..Rewards::default() };
    let fresh = Rewards { stage: Stage::NeedsVolume { required: dec("10000.0") }, ..Rewards::default() };
    assert_eq!(
        short.toward_code().map(|(_, bar)| bar),
        fresh.toward_code().map(|(_, bar)| bar),
        "one bar, whoever is measured against it"
    );
    assert_eq!(short.toward_code(), Some((dec("180.68"), dec("10000"))));
    assert_eq!(Rewards { stage: Stage::NeedsCode, ..Rewards::default() }.toward_code(), None);
}

#[test]
fn a_live_builder_s_pot_is_its_builder_fees_and_what_its_code_was_paid() {
    let rewards = read(LIVE);
    assert_eq!(rewards.builder, dec("82.23638757"));
    assert_eq!((rewards.unclaimed, rewards.claimed), (dec("0.0012616"), dec("442.9489995")));
    assert_eq!(rewards.pot(), dec("442.9502611"));
    assert_eq!(
        rewards.referral(),
        dec("360.71387353"),
        "the pot less the builder fees: what its one paying referral reports as rewarded to its referrer"
    );
    assert_eq!(rewards.stage, Stage::Ready { code: "DFU".to_owned() });
    assert!(rewards.stage.earning());
    assert_eq!(rewards.toward_code(), None);
}

#[test]
fn a_stage_this_build_does_not_know_is_kept_by_its_name() {
    let rewards = read(
        r#"{"cumVlm": "0.0", "unclaimedRewards": "0.0", "claimedRewards": "0.0",
            "referrerState": {"stage": "somethingNew"}}"#,
    );
    assert_eq!(rewards.stage, Stage::Other("somethingNew".to_owned()));
    assert_eq!(rewards.builder, Dec::ZERO, "an answer without builder rewards has none");
}
