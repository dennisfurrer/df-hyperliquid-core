//! Sub-deployer grants on the wire.

use super::*;
use crate::deploy::outcomes::actions::outcome_deploy::OutcomeDeploy;
use crate::deploy::outcomes::actions::support::{address, venue};
use crate::utils::sign::L1Action;

#[test]
fn a_grant_names_its_user_in_lower_case() {
    let grants = OutcomeDeploy::set_sub_deployers(
        venue("ab"),
        Permission::ALL
            .into_iter()
            .map(|permission| Grant {
                permission,
                user: address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414"),
                allowed: permission != Permission::SettleQuestion,
            })
            .collect(),
    )
    .unwrap();
    assert_eq!(
        grants.wire(),
        r#"{"type":"outcomeDeploy","venue":"ab","operation":{"setSubDeployers":[{"variant":"registerStandaloneOutcomeFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"registerQuestionFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"registerAndAssociateNamedOutcomeFromTemplate","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"settleOutcome","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":true},{"variant":"settleQuestion","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","allowed":false}]}}"#
    );
}
