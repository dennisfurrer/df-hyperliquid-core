//! `outcomeDeployerLimits` as the venue answers it.

use super::*;

#[test]
fn the_read_names_its_type() {
    assert_eq!(deployer_limits_request("out"), r#"{"type":"outcomeDeployerLimits","venue":"out"}"#);
}

/// Live, 2026-09-13, on a venue holding 99.
#[test]
fn the_deployer_limits_read_what_remains() {
    let remaining: Remaining =
        serde_json::from_str(r#"{"nDailyOutcomesRemaining":494,"nActiveOutcomesRemaining":1}"#).unwrap();
    assert_eq!(remaining, Remaining { daily: 494, active: 1 });
    assert_eq!(serde_json::from_str::<Remaining>("{}").unwrap(), Remaining { daily: 0, active: 0 });
}
