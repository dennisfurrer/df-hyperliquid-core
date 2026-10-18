//! Plans against the tokens in `spot_tokens.json` (live `spotMeta`, both networks, 2026-10-08):
//! the action an account takes, the key it needs, the one balance each move changes and by how
//! much, and every move refused before it is signed.

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::sign::UserAction;
use crate::utils::{Address, Dec};

use super::*;
use crate::transfer::send::actions::{UsdClassTransfer, UsdSend};
use crate::transfer::{Amount, HYPE_SYSTEM, PERP_DEX, SPOT_DEX, SpotOut, SpotToken, Withdraw3, system_address};

const TOKENS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/transfer/spot_tokens.json"));
const OWN: &str = "0x14791697260E4c9A71f18484C9f997B308e59325";
const FRIEND: &str = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414";
const NONCE: u64 = 1_790_412_105_001;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn amount(typed: &str) -> Amount {
    Amount::parse(typed).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn token(network: Network, name: &str) -> SpotToken {
    let all: BTreeMap<String, Vec<SpotToken>> = serde_json::from_str(TOKENS).unwrap();
    all[network.name()].iter().find(|token| token.name == name).cloned().unwrap()
}

fn ctx(network: Network, keeping: Keeping) -> Ctx {
    Ctx { network, account: address(OWN), keeping, nonce: NONCE }
}

fn effect(balance: Balance, change: Change) -> Effect {
    Effect { balance, change }
}

/// `spotSend` for a separate account; `sendAsset` from spot to spot for a pooled one, which
/// refuses `spotSend`, and for one whose mode is not read, since both kinds of account take it.
#[test]
fn a_spot_token_leaves_by_the_action_its_account_takes() {
    for (keeping, kind) in [
        (Keeping::Apart, "spotSend"),
        (Keeping::Native, "spotSend"),
        (Keeping::Unified, "sendAsset"),
        (Keeping::Portfolio, "sendAsset"),
        (Keeping::Unknown, "sendAsset"),
    ] {
        let out = SpotOut::new(address(FRIEND), "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec", amount("1"), keeping, NONCE);
        assert_eq!(out.user_signed().kind(), kind, "{keeping:?}");
        if kind == "sendAsset" {
            assert!(out.wire(Network::Mainnet).contains(r#""sourceDex":"spot","destinationDex":"spot","#));
        }
    }
}

/// Every user-signed action needs the account's own key; an agent signs L1 actions for its master.
#[test]
fn each_action_names_the_key_it_needs() {
    for kind in ["usdClassTransfer", "usdSend", "spotSend", "sendAsset", "withdraw3"] {
        assert_eq!(key(kind), Some(Signers::Main), "{kind}");
    }
    assert_eq!(key("agentSendAsset"), Some(Signers::Either));
    assert_eq!(key("order"), None, "not this crate's");
    assert!(Signers::Either.takes_agent() && !Signers::Main.takes_agent());
    let keys = [
        UsdClassTransfer::KEY,
        UsdSend::KEY,
        crate::transfer::SpotSend::KEY,
        crate::transfer::SendAsset::KEY,
        Withdraw3::KEY,
    ];
    assert_eq!(keys, [Signers::Main; 5]);
    assert_eq!(crate::transfer::AgentSendAsset::KEY, Signers::Either);
}

/// USDC leaving the perp side is read back where the account keeps it: the main dex's
/// `withdrawable`, or spot USDC for a pooled account. A move between two balances leaves the one
/// it comes from by exactly the amount; a send or a crossing by at least it, since a fresh
/// recipient's fee or HYPE for gas may come on top; a withdrawal's fee comes out of the amount.
#[test]
fn each_move_changes_one_balance_by_its_amount() {
    let main = || Balance::Perp(PERP_DEX.to_owned());
    let apart = ctx(Network::Mainnet, Keeping::Apart);
    let pooled = ctx(Network::Mainnet, Keeping::Unified);
    let unknown = ctx(Network::Mainnet, Keeping::Unknown);
    let testnet = ctx(Network::Testnet, Keeping::Apart);
    let usdc = token(Network::Testnet, "USDC");
    let between = testnet.between(PERP_DEX, SPOT_DEX, &usdc, amount("12.345678")).unwrap();
    assert!(between.action.wire(Network::Testnet).contains(concat!(
        r#""destination":"0x14791697260e4c9a71f18484c9f997b308e59325","sourceDex":"","destinationDex":"spot","#,
        r#""token":"USDC:0xeb62eee3685fc4c43992febcd9e75443","amount":"12.345678""#
    )));
    let mut cases = vec![
        (apart.class_transfer(amount("12.50"), true).unwrap().effect, effect(main(), Change::Up(dec("12.5")))),
        (apart.class_transfer(amount("2"), false).unwrap().effect, effect(main(), Change::Down(dec("2")))),
        (apart.usd_send(address(FRIEND), amount("7")).unwrap().effect, effect(main(), Change::DownAtLeast(dec("7")))),
        (unknown.usd_send(address(FRIEND), amount("7")).unwrap().effect, effect(main(), Change::DownAtLeast(dec("7")))),
        (
            pooled.usd_send(address(FRIEND), amount("7")).unwrap().effect,
            effect(Balance::Spot(0), Change::DownAtLeast(dec("7"))),
        ),
        (apart.withdraw(address(OWN), amount("25")).unwrap().effect, effect(main(), Change::Down(dec("25")))),
        (
            pooled.withdraw(address(OWN), amount("25")).unwrap().effect,
            effect(Balance::Spot(0), Change::Down(dec("25"))),
        ),
        (
            apart.send_token(address(FRIEND), &token(Network::Mainnet, "UBTC"), amount("0.00000001")).unwrap().effect,
            effect(Balance::Spot(197), Change::DownAtLeast(dec("0.00000001"))),
        ),
        (
            pooled.to_evm(&token(Network::Mainnet, "USDT0"), amount("1.000001")).unwrap().effect,
            effect(Balance::Spot(268), Change::DownAtLeast(dec("1.000001"))),
        ),
        (between.effect, effect(main(), Change::Down(dec("12.345678")))),
        (
            testnet.between(SPOT_DEX, "xyz", &usdc, amount("1")).unwrap().effect,
            effect(Balance::Spot(0), Change::Down(Dec::ONE)),
        ),
    ];
    for network in Network::ALL {
        let hype = token(network, "HYPE");
        let plan = ctx(network, Keeping::Apart).to_evm(&hype, amount("1.25")).unwrap();
        assert!(plan.action.wire(network).contains(&format!(r#""destination":"{HYPE_SYSTEM}""#)), "{network}");
        cases.push((plan.effect, effect(Balance::Spot(hype.index), Change::DownAtLeast(dec("1.25")))));
    }
    for (planned, expected) in cases {
        assert_eq!(planned, expected);
    }

    let up = effect(main(), Change::Up(dec("12.5")));
    assert!(
        up.landed(dec("100"), dec("112.5"))
            && !up.landed(dec("100"), dec("112.4"))
            && !up.landed(dec("100"), dec("100"))
    );
    let down = effect(main(), Change::Down(dec("25")));
    assert!(down.landed(dec("100"), dec("75")) && !down.landed(dec("100"), dec("74")));
    let at_least = effect(main(), Change::DownAtLeast(dec("7")));
    assert!(at_least.landed(dec("100"), dec("93")) && at_least.landed(dec("100"), dec("92")));
    assert!(!at_least.landed(dec("100"), dec("93.5")));
}

/// A pooled account has one balance to move between; a send goes to someone else, never through a
/// system address, which `to_evm` plans; an amount finer than its balance counts is refused, never
/// rounded (USDC on the perp side counts in millionths, a crossing in the fewer of its two sides'
/// places); and only a token linked to HyperEVM crosses.
#[test]
fn every_move_the_venue_would_refuse_or_round_is_refused_before_it_is_signed() {
    let apart = ctx(Network::Mainnet, Keeping::Apart);
    let pooled = ctx(Network::Mainnet, Keeping::Unified);
    let (kntq, usdc, usdt0) =
        (token(Network::Mainnet, "KNTQ"), token(Network::Mainnet, "USDC"), token(Network::Mainnet, "USDT0"));
    let fine = || amount("1.0000001");
    let too_fine = Refused::TooFine { amount: fine().value(), places: 6 };
    let cases: Vec<(Result<(), Refused>, Refused)> = vec![
        (pooled.class_transfer(amount("1"), true).map(drop), Refused::OneBalance("perps and spot")),
        (pooled.between(PERP_DEX, SPOT_DEX, &usdc, amount("1")).map(drop), Refused::OneBalance("dexes")),
        (pooled.agent_between(PERP_DEX, SPOT_DEX, &usdc, amount("1")).map(drop), Refused::OneBalance("dexes")),
        (apart.between(SPOT_DEX, SPOT_DEX, &usdc, amount("1")).map(drop), Refused::SameDex),
        (apart.agent_between("xyz", "xyz", &usdc, amount("1")).map(drop), Refused::SameDex),
        (apart.usd_send(address(OWN), amount("1")).map(drop), Refused::ToSelf),
        (apart.send_token(address(OWN), &kntq, amount("1")).map(drop), Refused::ToSelf),
        (apart.usd_send(HYPE_SYSTEM, amount("1")).map(drop), Refused::SystemAddress(HYPE_SYSTEM)),
        (
            apart.send_token(system_address(124), &kntq, amount("1")).map(drop),
            Refused::SystemAddress(system_address(124)),
        ),
        (apart.withdraw(HYPE_SYSTEM, amount("1")).map(drop), Refused::SystemAddress(HYPE_SYSTEM)),
        (apart.usd_send(address(FRIEND), fine()).map(drop), too_fine.clone()),
        (apart.withdraw(address(OWN), fine()).map(drop), too_fine.clone()),
        (apart.class_transfer(fine(), true).map(drop), too_fine.clone()),
        (apart.between(SPOT_DEX, PERP_DEX, &usdc, fine()).map(drop), too_fine.clone()),
        (pooled.to_evm(&usdt0, fine()).map(drop), too_fine),
        (apart.to_evm(&usdc, amount("1")).map(drop), Refused::Usdc),
        (
            ctx(Network::Testnet, Keeping::Apart).to_evm(&token(Network::Testnet, "ALERTT"), amount("1")).map(drop),
            Refused::NotLinked("ALERTT".to_owned()),
        ),
    ];
    for (planned, refused) in cases {
        assert_eq!(planned, Err(refused));
    }
}
