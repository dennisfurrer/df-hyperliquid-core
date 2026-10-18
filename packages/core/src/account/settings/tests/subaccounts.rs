//! Subaccounts: whose account an action is about, what a subaccount refuses, the vectors it signs
//! with as the vault, the money moves refused before signing, and the `subAccounts` answer. Opening
//! and funding one runs in the mocked journeys.

use crate::account::LocalKey;
use crate::client::Network;
use crate::utils::sign::L1Action;
use crate::utils::{AssetKind, Json, hex};

use super::*;
use crate::account::fixtures::{SUBS_UNIFIED, UNIFIED, UNIFIED_SUB, address, dec, read};
use crate::account::keys::signers::ActionKind;

const VAULT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/vault.json"));

/// The live shape: a desk with a short and a flat leg, and one never funded.
const SUBS: &str = r#"[
    {"name":"desk two",
     "subAccountUser":"0x1d9dd5f0f5d9d0bf5a0aa8de6a99c4d92a0b2dc4",
     "master":"0xbaff87940cb148c5e2529c0a5944b0a3386fb015",
     "clearinghouseState":{
        "marginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999",
            "totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},
        "crossMarginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999",
            "totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},
        "crossMaintenanceMarginUsed":"29231.7948","withdrawable":"1204.55",
        "assetPositions":[
            {"type":"oneWay","position":{"coin":"HYPE","szi":"-6267.0",
                "leverage":{"type":"cross","value":3},"entryPx":"13.1333",
                "positionValue":"584635.8959999999","unrealizedPnl":"-502329.38087",
                "marginUsed":"194878.632","maxLeverage":10}},
            {"type":"oneWay","position":{"coin":"BTC","szi":"0.0",
                "leverage":{"type":"cross","value":5},"entryPx":null,
                "positionValue":"0.0","unrealizedPnl":"0.0",
                "marginUsed":"0.0","maxLeverage":40}}]},
     "spotState":{"balances":[
        {"coin":"USDC","token":0,"total":"290383.20298955","hold":"194870.276","entryNtl":"0.0"},
        {"coin":"PURR","token":1,"total":"250814.34657","hold":"0.0","entryNtl":"54401.11170465"}]}},
    {"name":"fresh desk",
     "subAccountUser":"0x9c8a1dd0a4e8c7c8f7b3a7c1e6d5b4a39281f0e7",
     "master":"0xbaff87940cb148c5e2529c0a5944b0a3386fb015",
     "clearinghouseState":null,"spotState":null}]"#;

fn sub() -> AccountRef {
    AccountRef::sub(address(UNIFIED), address(UNIFIED_SUB))
}

/// The live shape (`SUBS`, a desk with a short and a flat leg, and one never funded) and the
/// unified account's own: a subaccount is read at its own address, traded as the vault, and signed
/// for by its master's key or the master's agent.
#[test]
fn a_subaccount_is_read_at_its_own_address_with_its_own_states_and_traded_as_the_vault() {
    let own = AccountRef::own(address(UNIFIED));
    assert_eq!((own.read_at(), own.vault_address(), own.signs_for()), (address(UNIFIED), None, address(UNIFIED)));
    assert!(!own.is_sub());
    let listed: Vec<SubAccount> = read(SUBS_UNIFIED);
    let sub = listed[0].account();
    assert_eq!((sub, sub.is_sub()), (AccountRef::sub(address(UNIFIED), address(UNIFIED_SUB)), true));
    assert_eq!(
        (sub.read_at(), sub.vault_address(), sub.signs_for()),
        (address(UNIFIED_SUB), Some(address(UNIFIED_SUB)), address(UNIFIED))
    );
    assert_eq!(listed[0].name, "SUPERVISED");
    assert_eq!(sub.to_string(), format!("{UNIFIED_SUB} (a subaccount of {UNIFIED})"));
    assert_eq!(own.to_string(), UNIFIED);

    let subs: Vec<SubAccount> = read(SUBS);
    let desk = &subs[0];
    assert_eq!(
        (desk.equity(), desk.notional(), desk.withdrawable()),
        (dec("41008.519458"), dec("584635.8959999999"), dec("1204.55"))
    );
    assert_eq!((desk.positions(), desk.spot_usdc()), (1, dec("290383.20298955")), "the flat BTC leg is not a position");
    let fresh = &subs[1];
    assert_eq!((fresh.clearinghouse_state.as_ref(), fresh.spot_state.as_ref()), (None, None), "null is no state");
    assert_eq!(
        (fresh.equity(), fresh.withdrawable(), fresh.positions(), fresh.spot_usdc()),
        (Dec::ZERO, Dec::ZERO, 0, Dec::ZERO)
    );
}

/// A subaccount's spot and HIP-4 orders, moves between its ledgers, approvals and a scheduled
/// cancel are refused rather than done as the master.
#[test]
fn what_a_subaccount_refuses_is_data() {
    let sub = sub();
    let refused = |kind: ActionKind, market: Option<AssetKind>| sub.refuses(kind, market);
    for market in [AssetKind::Perp, AssetKind::Hip3Perp] {
        for kind in [ActionKind::Order, ActionKind::Modify, ActionKind::BatchModify, ActionKind::TwapOrder] {
            assert_eq!(refused(kind, Some(market)), None, "{kind} on {market:?}");
        }
    }
    for kind in [
        ActionKind::Cancel,
        ActionKind::CancelByCloid,
        ActionKind::TwapCancel,
        ActionKind::UpdateLeverage,
        ActionKind::UpdateIsolatedMargin,
        ActionKind::Noop,
        ActionKind::UserSetAbstraction,
        ActionKind::AgentSetAbstraction,
    ] {
        assert_eq!(refused(kind, None), None, "{kind}");
    }
    assert_eq!(refused(ActionKind::Order, Some(AssetKind::Spot)), Some(SubRefusal::Spot));
    assert_eq!(refused(ActionKind::Order, Some(AssetKind::Outcome)), Some(SubRefusal::Outcome));
    assert_eq!(refused(ActionKind::Order, None), Some(SubRefusal::MarketUnknown));
    assert_eq!(refused(ActionKind::UserOutcome, None), Some(SubRefusal::Outcome));
    for kind in [ActionKind::UsdClassTransfer, ActionKind::AgentSendAsset] {
        assert_eq!(refused(kind, None), Some(SubRefusal::LedgerMove), "{kind}");
    }
    for kind in [ActionKind::ApproveAgent, ActionKind::ApproveBuilderFee] {
        assert_eq!(refused(kind, None), Some(SubRefusal::Approval), "{kind}");
    }
    assert_eq!(refused(ActionKind::ScheduleCancel, None), Some(SubRefusal::ScheduleCancel));
    for kind in [ActionKind::CreateSubAccount, ActionKind::SubAccountTransfer, ActionKind::SubAccountSpotTransfer] {
        assert_eq!(refused(kind, None), Some(SubRefusal::MastersOwn), "{kind}");
    }
    for kind in [ActionKind::UsdSend, ActionKind::SpotSend, ActionKind::Withdraw3, ActionKind::SendAsset] {
        assert_eq!(refused(kind, None), Some(SubRefusal::NoKey), "{kind}");
    }
    let own = AccountRef::own(address(UNIFIED));
    assert!(ActionKind::ALL.iter().all(|kind| own.refuses(*kind, Some(AssetKind::Spot)).is_none()));
    assert_eq!(SubRefusal::Spot.to_string(), "a subaccount places no spot orders here");
}

/// The official Python SDK, signing for subaccount `0x1719...75ea`: what a subaccount may do,
/// signed with it as `vaultAddress`, hashes and signs as recorded.
#[test]
fn a_subaccount_s_perp_actions_sign_with_it_as_the_vault() {
    let file = Json::parse(VAULT).unwrap();
    let text = |json: &Json, key: &str| json.get(key).and_then(Json::as_str).unwrap().to_owned();
    let mut signed = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let written = case.get("action").unwrap();
        // The SDK's own `dummy` test action is no kind of the venue's.
        let Some(kind) = written.get("type").and_then(Json::as_str).and_then(ActionKind::from_wire) else {
            continue;
        };
        let action = written.to_action().unwrap();
        let owner: Address = text(case, "address").parse().unwrap();
        let account = AccountRef::sub(owner, text(case, "vaultAddress").parse().unwrap());
        assert_eq!(account.refuses(kind, Some(AssetKind::Perp)), None, "{kind}");
        let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
        let vault = account.vault_address();
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, vault)), text(case, "actionHash"), "{kind}");
        let key = LocalKey::from_hex(&text(case, "key")).unwrap();
        assert_eq!(key.address(), owner);
        let signature = key.sign_digest(&action.digest(nonce, vault, Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        let word = |hex: String| format!("0x{:0>64}", hex.trim_start_matches("0x"));
        assert_eq!(signature.r_hex(), word(text(expected, "r")), "{kind}");
        assert_eq!(signature.s_hex(), word(text(expected, "s")), "{kind}");
        signed += 1;
    }
    assert_eq!(signed, 3, "two orders and a cancel");
}

/// Perp USDC moves in millionths, refused rather than rounded when finer; a spot token moves only
/// in an amount above nothing.
#[test]
fn a_transfer_finer_than_a_millionth_or_of_nothing_is_refused() {
    let sub = address(UNIFIED_SUB);
    for (usdc, taken) in [("5", true), ("0.000001", true), ("0.0000001", false), ("1.2345678", false)] {
        assert_eq!(SubAccountTransfer::deposit(sub, dec(usdc)).is_ok(), taken, "{usdc} in");
        assert_eq!(SubAccountTransfer::withdraw(sub, dec(usdc)).is_ok(), taken, "{usdc} out");
    }
    let hype = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";
    for (amount, taken) in [("2", true), ("0", false), ("-1", false)] {
        assert_eq!(SubAccountSpotTransfer::new(sub, true, hype, dec(amount)).is_ok(), taken, "{amount}");
    }
}
