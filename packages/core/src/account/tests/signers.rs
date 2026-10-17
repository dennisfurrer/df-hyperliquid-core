use super::*;
use crate::utils::json::Json;

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
const DESK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/desk.json"));

fn kinds(names: &[&str]) -> Vec<ActionKind> {
    names.iter().map(|name| ActionKind::from_wire(name).unwrap_or_else(|| panic!("{name} is a kind"))).collect()
}

#[test]
fn an_agent_signs_what_hip4_tui_lets_it_and_nothing_else() {
    let agent_may = kinds(&[
        "order",
        "cancel",
        "cancelByCloid",
        "modify",
        "batchModify",
        "twapOrder",
        "twapCancel",
        "updateLeverage",
        "updateIsolatedMargin",
        "vaultTransfer",
        "borrowLend",
        "agentSetAbstraction",
        "setReferrer",
    ]);
    for kind in agent_may {
        assert_eq!(may_sign(kind, Key::Agent), Ok(()), "{kind}");
        assert_eq!(kind.signers(), Signers::Either, "{kind}");
        assert_eq!(kind.why(), None, "{kind}");
    }
    for kind in kinds(&["createSubAccount", "subAccountTransfer", "subAccountSpotTransfer", "userOutcome"]) {
        assert_eq!((kind.signers(), kind.why()), (Signers::Main, Some(Why::WrongAccount)), "{kind}");
        assert_eq!(may_sign(kind, Key::Agent), Err(Refused { kind, why: Why::WrongAccount }), "{kind}");
        assert_eq!(may_sign(kind, Key::Main), Ok(()), "{kind}");
    }
}

#[test]
fn every_user_signed_action_takes_the_main_key_and_an_agent_cannot_approve_a_builder_fee() {
    let user_signed = kinds(&[
        "usdClassTransfer",
        "usdSend",
        "spotSend",
        "sendAsset",
        "withdraw3",
        "cDeposit",
        "cWithdraw",
        "tokenDelegate",
        "approveAgent",
        "approveBuilderFee",
        "userSetAbstraction",
    ]);
    for kind in user_signed {
        assert!(kind.is_user_signed(), "{kind}");
        assert_eq!((kind.signers(), kind.why()), (Signers::Main, Some(Why::UserSigned)), "{kind}");
        assert!(may_sign(kind, Key::Agent).is_err(), "{kind}");
    }
    let refused = may_sign(ActionKind::ApproveBuilderFee, Key::Agent).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "approveBuilderFee is signed by the account's own key: an agent's signature recovers to the agent, which holds nothing"
    );
}

/// Every action in `@nktkas/hyperliquid`'s vectors is user-signed exactly when the file lists it
/// under `userSigned`.
#[test]
fn the_sdk_s_vectors_agree_on_which_actions_are_user_signed() {
    let file = Json::parse(NKTKAS).unwrap();
    for (list, user_signed) in [("userSigned", true), ("l1", false)] {
        for case in file.get(list).and_then(Json::as_array).unwrap() {
            let name = case.get("name").and_then(Json::as_str).unwrap();
            let kind = ActionKind::from_wire(name).unwrap_or_else(|| panic!("{name} is a kind"));
            assert_eq!(kind.is_user_signed(), user_signed, "{name}");
            assert_eq!(kind.wire(), name);
        }
    }
}

/// A session made by hand (`vectors/desk.json`): each request says which key signed it, and each
/// is one that key may sign.
#[test]
fn the_desk_s_requests_are_signed_by_keys_that_may_sign_them() {
    let file = Json::parse(DESK).unwrap();
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
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

#[test]
fn every_kind_reads_back_from_its_own_name() {
    for kind in ActionKind::ALL {
        assert_eq!(ActionKind::from_wire(kind.wire()), Some(kind));
    }
    assert_eq!(ActionKind::from_wire("approveagent"), None, "names are as the venue spells them");
}

#[test]
fn only_either_takes_an_agent() {
    assert!(Signers::Either.takes_agent());
    assert!(!Signers::Main.takes_agent());
    for kind in ActionKind::ALL {
        assert_eq!(kind.signers().takes_agent(), may_sign(kind, Key::Agent).is_ok(), "{kind}");
        assert_eq!(kind.why().is_none(), kind.signers().takes_agent(), "{kind}");
        assert_eq!(may_sign(kind, Key::Main), Ok(()), "{kind}: the account's own key signs anything");
    }
}
