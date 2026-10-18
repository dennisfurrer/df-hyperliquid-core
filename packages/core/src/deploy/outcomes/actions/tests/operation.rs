//! Each operation's key, body and the grant a sub-deployer needs to send it, and every grant on the
//! wire. The deployer vectors made outside this crate are in `tests/conformance/hip4.rs`.

use super::*;
use crate::deploy::outcomes::actions::outcome_deploy::OutcomeDeploy;
use crate::deploy::outcomes::actions::support::*;
use crate::utils::sign::L1Action;

#[test]
fn each_operation_names_the_grant_a_sub_deployer_needs_for_it() {
    let cases = [
        (python_standalone(), Some(Permission::RegisterStandalone), false),
        (python_question(), Some(Permission::RegisterQuestion), false),
        (python_associate(), Some(Permission::RegisterAndAssociate), false),
        (python_settle_outcome(), Some(Permission::SettleOutcome), true),
        (python_settle_question(), Some(Permission::SettleQuestion), true),
        (python_sub_deployers(), None, false),
    ];
    for (action, permission, settles) in cases {
        let operation = &action.operation;
        assert_eq!(operation.permission(), permission, "{}", operation.wire_name());
        assert_eq!(operation.is_settlement(), settles, "{}", operation.wire_name());
        assert_eq!(
            action.action().get("operation"),
            Some(&Value::Map(Map::new().put(operation.wire_name(), operation.body())))
        );
    }
    assert_eq!(python_settle_question().operation.wire_name(), "settleQuestion2");
    assert_eq!(Permission::SettleQuestion.wire(), "settleQuestion", "the older name");
}

/// Every permission by the name the venue gives it, the user in lower case.
#[test]
fn a_grant_names_its_permission_and_its_user_in_lower_case() {
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
