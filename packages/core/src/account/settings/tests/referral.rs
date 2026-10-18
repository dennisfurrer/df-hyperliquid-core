//! `referral` as the venue answers it: the builder's on both networks, a fresh account's, and one
//! referred under a code; and the codes `setReferrer` takes. The read runs in the mocked journeys.

use super::*;
use crate::account::fixtures::{BUILDER_REFERRAL, BUILDER_REFERRAL_TESTNET, address, dec, read};

const FRESH: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/referral_fresh_mainnet.json"));
/// Live, mainnet, 2026-10-06: an account trading under `TESTCODE`, its own code not yet made.
const REFERRED: &str = r#"{"referredBy":{"referrer":"0x677d831aef5328190852e24f13c46cac05f984e7","code":"TESTCODE"},"cumVlm":"2547126.0","unclaimedRewards":"0.0","claimedRewards":"0.0","builderRewards":"0.0","referrerState":{"stage":"needToCreateCode"},"rewardHistory":[],"tokenToState":[[0,{"cumVlm":"2547126.0","unclaimedRewards":"0.0","claimedRewards":"0.0","builderRewards":"0.0"}]]}"#;

/// The builder owns `DFU` on mainnet and trades under `DEFIAPP`; on testnet its code is another, so
/// a code is per network. Codes are compared without case.
#[test]
fn a_referral_says_who_referred_whom_and_the_account_s_own_code() {
    let builder: Referral = read(BUILDER_REFERRAL);
    assert_eq!((builder.own_code(), builder.referred_code()), (Some("DFU"), Some("DEFIAPP")));
    assert_eq!(builder.referrer(), Some(address("0x1922810825c90f4270048b96da7b1803cd8609ef")));
    assert!(builder.owns("DFU") && builder.owns("dfu") && !builder.owns("DEFIAPP"));
    assert_eq!(builder.builder_rewards, dec("82.23638757"));
    let data = builder.referrer_state.data.as_ref().unwrap();
    assert_eq!((data.n_referrals, data.referral_states.len()), (Some(2), 2));
    let first = &data.referral_states[0];
    assert_eq!(first.user, address("0x047a1c608406972d7c26132f189bbfb0cfefd413"));
    assert_eq!((first.cum_vlm, first.cum_fees_rewarded_to_referrer), (dec("9237220.0999999996"), dec("360.71387353")));
    assert_eq!(first.time_joined, 1_778_671_901_298);
    let testnet: Referral = read(BUILDER_REFERRAL_TESTNET);
    assert_eq!((testnet.own_code(), testnet.referred_code(), testnet.owns("DFU")), (Some("OUTCOME"), None, false));
    assert_eq!(testnet.referrer_state.data.as_ref().and_then(|data| data.n_referrals), Some(168));

    for (said, stage, required, referrer) in [
        (FRESH, "needToTrade", Some(dec("10000")), None),
        (REFERRED, "needToCreateCode", None, Some(address("0x677d831aef5328190852e24f13c46cac05f984e7"))),
    ] {
        let referral: Referral = read(said);
        assert_eq!(referral.referrer_state.stage, stage);
        assert_eq!(referral.referrer_state.data.as_ref().and_then(|data| data.required), required, "{stage}");
        assert_eq!((referral.referrer(), referral.own_code()), (referrer, None), "{stage}");
    }
}

/// The figures by quote token: USDC's are the ones the answer's top level carries; the builder's
/// second token (360 on mainnet, 1452 on testnet) has its own.
#[test]
fn each_quote_token_s_rewards_are_read_apart() {
    let referral: Referral = read(BUILDER_REFERRAL);
    let tokens: Option<Vec<u32>> =
        referral.token_to_state.as_ref().map(|states| states.iter().map(|(token, _)| *token).collect());
    assert_eq!(tokens, Some(vec![0, 360]));
    let usdc = referral.token(0).unwrap();
    assert_eq!(
        (usdc.cum_vlm, usdc.unclaimed_rewards, usdc.claimed_rewards, usdc.builder_rewards),
        (referral.cum_vlm, referral.unclaimed_rewards, referral.claimed_rewards, referral.builder_rewards)
    );
    let other = referral.token(360).unwrap();
    assert_eq!((other.claimed_rewards, other.builder_rewards), (dec("14.4985363"), dec("14.4985363")));
    assert_eq!(referral.token(1), None);
    let testnet: Referral = read(BUILDER_REFERRAL_TESTNET);
    assert_eq!(testnet.token(1452).map(|state| state.unclaimed_rewards), Some(dec("35.16606452")));
    let trimmed: Referral = read(
        r#"{"cumVlm":"0.0","unclaimedRewards":"0.0","claimedRewards":"0.0","referrerState":{"stage":"needToCreateCode"}}"#,
    );
    assert_eq!((trimmed.token_to_state.as_ref(), trimmed.token(0)), (None, None), "not said, not read as none");
}

#[test]
fn a_code_is_one_to_twenty_characters() {
    for (code, taken) in [("DFU", true), ("x", true), (&*"x".repeat(20), true), ("", false), (&*"x".repeat(21), false)]
    {
        let set = SetReferrer::new(code);
        assert_eq!(set.is_ok(), taken, "{code}");
        if !taken {
            assert_eq!(set, Err(BadCode(code.to_owned())));
        }
    }
}
