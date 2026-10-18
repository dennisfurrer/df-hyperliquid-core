//! Each operation's key, body and the grant a sub-deployer needs to send it.

use super::*;
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
    assert_eq!(Permission::SettleQuestion.wire(), "settleQuestion");
}
