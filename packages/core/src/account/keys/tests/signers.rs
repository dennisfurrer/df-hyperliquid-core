//! Who may sign each action, by its rule, and the other implementations' files agreeing.

use super::*;
use crate::utils::json::Json;

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
const SESSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/session.json"));

/// User-signed actions recover to the agent, which holds nothing; subaccount actions and HIP-4
/// splits would open or drain the wrong account; an agent signs every other L1 action for its
/// master, and the account's own key signs anything.
#[test]
fn each_kind_is_signed_by_the_keys_its_rule_allows() {
    use Why::{UserSigned, WrongAccount};
    let rules: [(&str, Option<Why>); 31] = [
        ("order", None),
        ("cancel", None),
        ("cancelByCloid", None),
        ("modify", None),
        ("batchModify", None),
        ("twapOrder", None),
        ("twapCancel", None),
        ("updateLeverage", None),
        ("updateIsolatedMargin", None),
        ("scheduleCancel", None),
        ("noop", None),
        ("agentSetAbstraction", None),
        ("agentSendAsset", None),
        ("setReferrer", None),
        ("createSubAccount", Some(WrongAccount)),
        ("subAccountTransfer", Some(WrongAccount)),
        ("subAccountSpotTransfer", Some(WrongAccount)),
        ("vaultTransfer", None),
        ("borrowLend", None),
        ("userOutcome", Some(WrongAccount)),
        ("usdClassTransfer", Some(UserSigned)),
        ("usdSend", Some(UserSigned)),
        ("spotSend", Some(UserSigned)),
        ("sendAsset", Some(UserSigned)),
        ("withdraw3", Some(UserSigned)),
        ("cDeposit", Some(UserSigned)),
        ("cWithdraw", Some(UserSigned)),
        ("tokenDelegate", Some(UserSigned)),
        ("approveAgent", Some(UserSigned)),
        ("approveBuilderFee", Some(UserSigned)),
        ("userSetAbstraction", Some(UserSigned)),
    ];
    for ((wire, why), kind) in rules.into_iter().zip(ActionKind::ALL) {
        assert_eq!((kind.wire(), ActionKind::from_wire(wire)), (wire, Some(kind)), "{wire}");
        assert_eq!((kind.why(), kind.is_user_signed()), (why, why == Some(UserSigned)), "{kind}");
        let signers = if why.is_some() { Signers::Main } else { Signers::Either };
        assert_eq!((kind.signers(), signers.takes_agent()), (signers, why.is_none()), "{kind}");
        let refused = why.map(|why| Refused { kind, why });
        assert_eq!(may_sign(kind, Key::Agent).err(), refused, "{kind}");
        assert_eq!(may_sign(kind, Key::Main), Ok(()), "{kind}: the account's own key signs anything");
    }
    assert_eq!(ActionKind::from_wire("approveagent"), None, "names are as the venue spells them");
    assert_eq!(
        may_sign(ActionKind::ApproveBuilderFee, Key::Agent).unwrap_err().to_string(),
        "approveBuilderFee is signed by the account's own key: an agent's signature recovers to the agent, which holds nothing"
    );
}

/// `@nktkas/hyperliquid` lists an action under `userSigned` exactly when it is user-signed, and in
/// the session made by hand (`vectors/session.json`) each request is signed by a key that may.
#[test]
fn the_other_implementations_agree_on_who_signs_what() {
    let file = Json::parse(NKTKAS).unwrap();
    for (list, user_signed) in [("userSigned", true), ("l1", false)] {
        for case in file.get(list).and_then(Json::as_array).unwrap() {
            let name = case.get("name").and_then(Json::as_str).unwrap();
            let kind = ActionKind::from_wire(name).unwrap_or_else(|| panic!("{name} is a kind"));
            assert_eq!(kind.is_user_signed(), user_signed, "{name}");
        }
    }
    let session = Json::parse(SESSION).unwrap();
    let cases = session.get("cases").and_then(Json::as_array).unwrap();
    assert!(cases.len() > 10);
    for case in cases {
        let kind = case.get("action").and_then(|action| action.get("type")).and_then(Json::as_str).unwrap();
        let kind = ActionKind::from_wire(kind).unwrap_or_else(|| panic!("{kind} is a kind"));
        let key = match case.get("by").and_then(Json::as_str) {
            Some("owner") => Key::Main,
            Some("agent") => Key::Agent,
            other => panic!("by {other:?}"),
        };
        assert_eq!(may_sign(kind, key), Ok(()), "{kind} by {key:?}");
    }
}
