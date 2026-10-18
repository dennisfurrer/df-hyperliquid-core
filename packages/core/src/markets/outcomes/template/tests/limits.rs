//! `outcomeDeployerLimits` as the venue answered it on 2026-09-13, for a venue holding 99.
//! Reads through a client are in `tests/mocked/markets`.

use super::*;

#[test]
fn the_deployer_limits_read_what_remains_and_nothing_as_none() {
    let remaining: Remaining =
        serde_json::from_str(r#"{"nDailyOutcomesRemaining":494,"nActiveOutcomesRemaining":1}"#).unwrap();
    assert_eq!(remaining, Remaining { daily: 494, active: 1 });
    assert_eq!(serde_json::from_str::<Remaining>("{}").unwrap(), Remaining::default());
}
