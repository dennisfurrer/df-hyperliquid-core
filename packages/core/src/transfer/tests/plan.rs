//! Plans against the tokens in `spot_tokens.json` (live `spotMeta`, both networks, 2026-10-08).

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::sign::UserAction;
use crate::utils::{Address, Dec};

use super::*;
use crate::transfer::actions::{UsdClassTransfer, UsdSend};
use crate::transfer::check::system_address;
use crate::transfer::keeping::Keeping;
use crate::transfer::{Amount, HYPE_SYSTEM, PERP_DEX, SPOT_DEX, SpotOut, SpotToken, Withdraw3};

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

#[test]
fn only_unified_and_portfolio_margin_pool_spot() {
    assert_eq!(Keeping::of("unifiedAccount"), Keeping::Pooled);
    assert_eq!(Keeping::of("portfolioMargin"), Keeping::Pooled);
    for apart in ["disabled", "default", "dexAbstraction"] {
        assert_eq!(Keeping::of(apart), Keeping::Apart, "{apart}");
    }
    for unknown in ["", "somethingNew"] {
        assert_eq!(Keeping::of(unknown), Keeping::Unknown, "{unknown}");
    }
    assert!(Keeping::Pooled.pools() && !Keeping::Apart.pools() && !Keeping::Unknown.pools());
}

/// `spotSend` for a separate account; `sendAsset` from spot to spot for a
/// pooled one, which refuses `spotSend`, and for one whose mode is not read,
/// since both kinds of account take it.
#[test]
fn a_spot_token_leaves_by_the_action_its_account_takes() {
    for (keeping, kind) in
        [(Keeping::Apart, "spotSend"), (Keeping::Pooled, "sendAsset"), (Keeping::Unknown, "sendAsset")]
    {
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
        assert_eq!(key(kind), Some(Key::Main), "{kind}");
    }
    assert_eq!(key("agentSendAsset"), Some(Key::Either));
    assert_eq!(key("order"), None, "not this crate's");
    assert!(Key::Either.takes_agent() && !Key::Main.takes_agent());
    assert_eq!(
        [
            UsdClassTransfer::KEY,
            UsdSend::KEY,
            crate::transfer::SpotSend::KEY,
            crate::transfer::SendAsset::KEY,
            Withdraw3::KEY
        ],
        [Key::Main; 5]
    );
    assert_eq!(crate::transfer::AgentSendAsset::KEY, Key::Either);
}

#[test]
fn a_class_transfer_moves_the_perp_balance_by_exactly_the_amount() {
    let plan = ctx(Network::Testnet, Keeping::Apart).class_transfer(amount("12.50"), true).unwrap();
    assert_eq!((plan.network, plan.nonce, plan.key), (Network::Testnet, NONCE, Key::Main));
    assert_eq!(plan.effect, Effect { balance: Balance::Perp(PERP_DEX.to_owned()), change: Change::Up(dec("12.5")) });
    assert!(plan.effect.landed(dec("100"), dec("112.5")));
    assert!(!plan.effect.landed(dec("100"), dec("112.4")), "short of the amount");
    assert!(!plan.effect.landed(dec("100"), dec("100")), "not landed yet");
    let back = ctx(Network::Mainnet, Keeping::Apart).class_transfer(amount("2"), false).unwrap();
    assert_eq!(back.effect.change, Change::Down(dec("2")));
    assert!(back.effect.landed(dec("10"), dec("8")));
}

/// A pooled account keeps one USDC balance, so nothing moves between its
/// perps and spot, or between its dexes.
#[test]
fn a_pooled_account_has_nothing_to_move_between() {
    let pooled = ctx(Network::Mainnet, Keeping::Pooled);
    assert_eq!(pooled.class_transfer(amount("1"), true), Err(Refused::OneBalance("perps and spot")));
    let usdc = token(Network::Mainnet, "USDC");
    assert_eq!(pooled.between(PERP_DEX, SPOT_DEX, &usdc, amount("1")), Err(Refused::OneBalance("dexes")));
    assert_eq!(pooled.agent_between(PERP_DEX, SPOT_DEX, &usdc, amount("1")), Err(Refused::OneBalance("dexes")));
}

/// USDC leaving the perp side is read back where it sits: the main dex's
/// `withdrawable` for a separate account, spot USDC for a pooled one.
#[test]
fn usdc_out_is_read_back_where_the_account_keeps_it() {
    for (keeping, balance) in [
        (Keeping::Apart, Balance::Perp(PERP_DEX.to_owned())),
        (Keeping::Unknown, Balance::Perp(PERP_DEX.to_owned())),
        (Keeping::Pooled, Balance::Spot(0)),
    ] {
        let ctx = ctx(Network::Mainnet, keeping);
        let send = ctx.usd_send(address(FRIEND), amount("7")).unwrap();
        assert_eq!(send.effect, Effect { balance: balance.clone(), change: Change::DownAtLeast(dec("7")) });
        let out = ctx.withdraw(address(OWN), amount("25")).unwrap();
        assert_eq!(out.effect, Effect { balance, change: Change::Down(dec("25")) }, "the fee comes out of the amount");
    }
    let send = ctx(Network::Mainnet, Keeping::Apart).usd_send(address(FRIEND), amount("7")).unwrap();
    assert!(send.effect.landed(dec("100"), dec("93")));
    assert!(send.effect.landed(dec("100"), dec("92")), "a fresh recipient's activation fee on top");
    assert!(!send.effect.landed(dec("100"), dec("93.5")));
}

#[test]
fn a_send_goes_to_someone_else_and_never_to_a_system_address() {
    let ctx = ctx(Network::Mainnet, Keeping::Apart);
    assert_eq!(ctx.usd_send(address(OWN), amount("1")), Err(Refused::ToSelf));
    assert_eq!(ctx.usd_send(HYPE_SYSTEM, amount("1")), Err(Refused::SystemAddress(HYPE_SYSTEM)));
    let kntq = token(Network::Mainnet, "KNTQ");
    assert_eq!(ctx.send_token(address(OWN), &kntq, amount("1")), Err(Refused::ToSelf));
    assert_eq!(
        ctx.send_token(system_address(124), &kntq, amount("1")),
        Err(Refused::SystemAddress(system_address(124))),
        "a crossing is `to_evm`"
    );
    assert_eq!(ctx.withdraw(HYPE_SYSTEM, amount("1")), Err(Refused::SystemAddress(HYPE_SYSTEM)));
}

/// USDC on the perp side counts in millionths; a spot token in its own
/// `weiDecimals`. Finer is refused, never rounded.
#[test]
fn an_amount_finer_than_its_balance_counts_is_refused() {
    let ctx = ctx(Network::Mainnet, Keeping::Apart);
    let fine = amount("1.0000001");
    assert_eq!(ctx.usd_send(address(FRIEND), fine.clone()), Err(Refused::TooFine { amount: fine.value(), places: 6 }));
    assert_eq!(ctx.withdraw(address(OWN), fine.clone()), Err(Refused::TooFine { amount: fine.value(), places: 6 }));
    assert_eq!(ctx.class_transfer(fine.clone(), true), Err(Refused::TooFine { amount: fine.value(), places: 6 }));
    let ubtc = token(Network::Mainnet, "UBTC");
    let plan = ctx.send_token(address(FRIEND), &ubtc, amount("0.00000001")).unwrap();
    assert_eq!(plan.effect, Effect { balance: Balance::Spot(197), change: Change::DownAtLeast(dec("0.00000001")) });
    let usdc = token(Network::Mainnet, "USDC");
    assert_eq!(
        ctx.between(SPOT_DEX, PERP_DEX, &usdc, fine.clone()),
        Err(Refused::TooFine { amount: fine.value(), places: 6 }),
        "a perp dex counts in millionths"
    );
}

#[test]
fn between_two_balances_is_a_send_asset_to_oneself_from_the_one_it_leaves() {
    let ctx = ctx(Network::Testnet, Keeping::Apart);
    let usdc = token(Network::Testnet, "USDC");
    let plan = ctx.between(PERP_DEX, SPOT_DEX, &usdc, amount("12.345678")).unwrap();
    assert_eq!(plan.key, Key::Main);
    assert!(plan.action.wire(Network::Testnet).contains(concat!(
        r#""destination":"0x14791697260e4c9a71f18484c9f997b308e59325","sourceDex":"","destinationDex":"spot","#,
        r#""token":"USDC:0xeb62eee3685fc4c43992febcd9e75443","amount":"12.345678""#
    )));
    assert_eq!(plan.effect, Effect { balance: Balance::Perp(String::new()), change: Change::Down(dec("12.345678")) });
    let back = ctx.between(SPOT_DEX, "xyz", &usdc, amount("1")).unwrap();
    assert_eq!(back.effect.balance, Balance::Spot(0));
    assert_eq!(ctx.between(SPOT_DEX, SPOT_DEX, &usdc, amount("1")), Err(Refused::SameDex));
    assert_eq!(ctx.agent_between("xyz", "xyz", &usdc, amount("1")), Err(Refused::SameDex));
}

/// The system address is the network's own HYPE (`0x2222...`), or `0x20` and
/// the token's index; the amount keeps at most the fewer of the token's Core
/// and HyperEVM decimals.
#[test]
fn a_token_crosses_to_hyperevm_through_its_own_system_address() {
    for network in Network::ALL {
        let hype = token(network, "HYPE");
        let plan = ctx(network, Keeping::Apart).to_evm(&hype, amount("1.25")).unwrap();
        assert!(
            plan.action.wire(network).contains(r#""destination":"0x2222222222222222222222222222222222222222""#),
            "{network}"
        );
        assert_eq!(
            plan.effect,
            Effect { balance: Balance::Spot(hype.index), change: Change::DownAtLeast(dec("1.25")) }
        );
    }
    let mainnet = ctx(Network::Mainnet, Keeping::Pooled);
    let ubtc = token(Network::Mainnet, "UBTC");
    let plan = mainnet.to_evm(&ubtc, amount("0.00000001")).unwrap();
    assert!(
        plan.action.wire(Network::Mainnet).contains(r#""destination":"0x20000000000000000000000000000000000000c5""#)
    );
    assert_eq!(ubtc.crossing_places(Network::Mainnet), Some(8), "ten on Core, eight on HyperEVM");
    let usdt0 = token(Network::Mainnet, "USDT0");
    let fine = amount("1.0000001");
    assert_eq!(
        mainnet.to_evm(&usdt0, fine.clone()),
        Err(Refused::TooFine { amount: fine.value(), places: 6 }),
        "eight on Core, six on HyperEVM"
    );
    assert!(mainnet.to_evm(&usdt0, amount("1.000001")).is_ok());
    assert_eq!(mainnet.to_evm(&token(Network::Mainnet, "USDC"), amount("1")), Err(Refused::Usdc));
    let alertt = token(Network::Testnet, "ALERTT");
    assert_eq!(
        ctx(Network::Testnet, Keeping::Apart).to_evm(&alertt, amount("1")),
        Err(Refused::NotLinked("ALERTT".to_owned())),
        "testnet's token 150 is not HYPE and has no ERC-20"
    );
}
