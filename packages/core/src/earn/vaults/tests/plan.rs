//! Plans against the recorded vaults in `details.json` and `equities.json` (2026-10-08): what each
//! move changes, and what is refused before signing.

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::sign::L1Action;
use crate::utils::{BadUnits, Dec};

use super::*;
use crate::earn::vaults::reads::{VaultDetails, VaultEquity};
use crate::earn::vaults::transfer::{TESTNET_HLP, VaultTransfer};

const DETAILS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/details.json"));
const EQUITIES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/equities.json"));
const NONCE: u64 = 1_790_412_105_001;
const DAY: u64 = 86_400_000;

fn details(name: &str) -> VaultDetails {
    let all: BTreeMap<String, VaultDetails> = serde_json::from_str(DETAILS).unwrap();
    all[name].clone()
}

/// The testnet follower's HLP share: 1068.95511, locked until 1765926140668.
fn equity() -> VaultEquity {
    let all: BTreeMap<String, Vec<VaultEquity>> = serde_json::from_str(EQUITIES).unwrap();
    all["testnetHlpFollower"][0].clone()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// A vault's equity moves with its profit and loss too, so a read back shows the direction. A
/// deposit is locked for the vault's lockup; a whole withdrawal is the equity cut to millionths, so
/// it never asks for more. An agent may sign either for its master.
#[test]
fn each_move_names_its_vault_its_direction_and_when_a_deposit_is_free() {
    assert!(VaultTransfer::KEY.takes_agent());
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: NONCE };
    let punks = details("testnetHyperpunks");
    let plan = ctx.deposit(&punks, dec("10")).unwrap();
    assert_eq!(plan.effect, Effect { balance: Balance::Vault(punks.vault_address), change: Change::Rises(dec("10")) });
    assert_eq!((plan.key, plan.free_at), (Signers::Either, Some(NONCE + 20_000)));
    assert!(plan.action.wire().contains(r#""isDeposit":true,"usd":10000000"#));
    let hlp = ctx.deposit(&details("testnetHlp"), dec("10")).unwrap();
    assert_eq!(hlp.free_at, Some(NONCE + 4 * DAY));
    assert!(hlp.effect.landed(dec("1068.9"), dec("1078.95")) && !hlp.effect.landed(dec("1068.9"), dec("1068.9")));
    let mainnet = Ctx { network: Network::Mainnet, nonce: NONCE, now_ms: NONCE };
    assert_eq!(mainnet.deposit(&details("mainnetGrowi"), dec("10")).unwrap().free_at, Some(NONCE + DAY));

    let equity = equity();
    let ctx = Ctx { now_ms: equity.locked_until_timestamp, ..ctx };
    let some = ctx.withdraw(&equity, dec("100")).unwrap();
    assert_eq!(some.effect, Effect { balance: Balance::Vault(TESTNET_HLP), change: Change::Falls(dec("100")) });
    assert_eq!((some.key, some.free_at), (Signers::Either, None));
    assert!(some.effect.landed(dec("1068.95511"), dec("968.95")));
    let all = ctx.withdraw_all(&equity).unwrap();
    assert_eq!(all.action.usd(), 1_068_955_110);
    assert!(
        all.action.wire().contains(r#""vaultAddress":"0xa15099a30bbf2e68942d6f4c43d70d04faeab0a0","isDeposit":false"#)
    );
    assert_eq!(all.effect.change, Change::Falls(dec("1068.95511")));
    assert_eq!(VaultTransfer::withdraw(TESTNET_HLP, dec("604934.164661")).unwrap().usd(), 604_934_164_661);
}

/// A vault closed or shut to deposits, a withdrawal before the lockup or of more than the equity,
/// and USDC finer than millionths (an equity uncut) or not above zero.
#[test]
fn what_a_vault_would_refuse_is_refused_before_signing() {
    let ctx = Ctx { network: Network::Mainnet, nonce: NONCE, now_ms: NONCE };
    let mut closed = details("mainnetGrowi");
    closed.is_closed = true;
    let mut shut = details("mainnetGrowi");
    shut.allow_deposits = false;
    let equity = equity();
    let until = equity.locked_until_timestamp;
    let early = Ctx { now_ms: until - 1, ..ctx };
    let late = Ctx { now_ms: until, ..ctx };
    let cases: Vec<(Result<(), Refused>, Refused)> = vec![
        (ctx.deposit(&closed, dec("10")).map(drop), Refused::NoDeposits(closed.vault_address)),
        (ctx.deposit(&shut, dec("10")).map(drop), Refused::NoDeposits(shut.vault_address)),
        (early.withdraw(&equity, dec("1")).map(drop), Refused::Locked { until }),
        (early.withdraw_all(&equity).map(drop), Refused::Locked { until }),
        (
            late.withdraw(&equity, dec("1068.955111")).map(drop),
            Refused::MoreThan { amount: dec("1068.955111"), held: dec("1068.95511") },
        ),
        (
            ctx.deposit(&details("mainnetGrowi"), dec("1.0000001")).map(drop),
            Refused::Units(BadUnits::TooFine { value: dec("1.0000001"), places: 6 }),
        ),
        (
            late.withdraw(&equity, dec("1068.9551109999")).map(drop),
            Refused::Units(BadUnits::TooFine { value: dec("1068.9551109999"), places: 6 }),
        ),
        (ctx.deposit(&details("mainnetGrowi"), Dec::ZERO).map(drop), Refused::Units(BadUnits::NotPositive(Dec::ZERO))),
    ];
    for (planned, refused) in cases {
        assert_eq!(planned, Err(refused));
    }
}
