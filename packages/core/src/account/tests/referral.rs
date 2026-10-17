use super::*;
use crate::account::fixtures::{
    BUILDER, BUILDER_REFERRAL, BUILDER_REFERRAL_TESTNET, Recorded, address, block_on, dec, read,
};

/// The builder owns `DFU` on mainnet and trades under `DEFIAPP`.
#[test]
fn the_live_builder_owns_its_code_and_trades_under_another() {
    let referral: Referral = read(BUILDER_REFERRAL);
    assert_eq!(referral.referred_code(), Some("DEFIAPP"));
    assert_eq!(referral.referrer(), Some(address("0x1922810825c90f4270048b96da7b1803cd8609ef")));
    assert_eq!(referral.own_code(), Some("DFU"));
    assert!(referral.owns("DFU"));
    assert!(referral.owns("dfu"), "codes are compared without case");
    assert!(!referral.owns("DEFIAPP"));
    assert_eq!(referral.builder_rewards, dec("82.23638757"));
    let data = referral.referrer_state.data.as_ref().unwrap();
    assert_eq!(data.n_referrals, Some(2));
    let first = &data.referral_states[0];
    assert_eq!(first.user, address("0x047a1c608406972d7c26132f189bbfb0cfefd413"));
    assert_eq!((first.cum_vlm, first.cum_fees_rewarded_to_referrer), (dec("9237220.0999999996"), dec("360.71387353")));
    assert_eq!(first.time_joined, 1_778_671_901_298);
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

/// On testnet the same account's code is another one, so `DFU` is someone
/// else's there: a code is per network.
#[test]
fn a_code_is_per_network() {
    let referral: Referral = read(BUILDER_REFERRAL_TESTNET);
    assert_eq!((referral.own_code(), referral.referred_code()), (Some("OUTCOME"), None));
    assert!(!referral.owns("DFU"));
    assert_eq!(referral.referrer_state.data.as_ref().and_then(|data| data.n_referrals), Some(168));
}

#[test]
fn a_referral_is_read_at_the_address() {
    let body = format!(r#"{{"type":"referral","user":"{BUILDER}"}}"#);
    assert_eq!(referral_request(address(BUILDER)), body);
    let transport = Recorded::new(&[(&body, BUILDER_REFERRAL)]);
    assert_eq!(block_on(referral(&transport, address(BUILDER))).unwrap().own_code(), Some("DFU"));
}
