//! What an account is worth, counted once: prices from the pairs' mids, each holding in its own
//! slice, and several accounts that hold each other. The live unified and standard accounts, held
//! against the venue's own `portfolio`, run in the mocked journeys.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;
use crate::account::fixtures::{PRICES, SPOT_PORTFOLIO_MARGIN, STANDARD, UNIFIED, address, dec, read};
use crate::account::holdings::perps::{ClearinghouseState, MarginSummary};
use crate::account::holdings::spot::{SpotBalance, SpotState};
use crate::account::holdings::standing::DexState;
use crate::account::settings::mode::Abstraction;

/// The recorded mids read as exact decimals, as a store holds them.
#[derive(Deserialize)]
struct Recorded {
    pairs: BTreeMap<String, String>,
    mids: BTreeMap<String, Dec>,
}

/// A token at its pair's mid, an outcome side `+N` at the mid quoted as `#N`, USDC at one; a mid of
/// nothing is no price.
#[test]
fn prices_come_from_the_pairs_and_exact_mids() {
    let recorded: Recorded = read(PRICES);
    let mut mids = recorded.mids;
    mids.insert("#14730".to_owned(), dec("0.584"));
    mids.insert("@142".to_owned(), Dec::ZERO);
    let prices = Prices::of(&recorded.pairs, &mids);
    for (coin, price) in [
        ("USDC", Some("1")),
        ("HYPE", Some("83.4515")),
        ("PURR", Some("0.11179")),
        ("+14730", Some("0.584")),
        ("UBTC", None),
        ("NOPE", None),
    ] {
        assert_eq!(prices.price(coin), price.map(dec), "{coin}");
    }
    assert_eq!(Prices::new().with("#14730", dec("0.584")).price("+14730"), Some(dec("0.584")));
}

fn usdc(total: &str) -> SpotBalance {
    SpotBalance { coin: "USDC".to_owned(), token: Some(0), total: dec(total), ..SpotBalance::default() }
}

fn spot(balances: Vec<SpotBalance>) -> Option<SpotState> {
    Some(SpotState { balances, ..SpotState::default() })
}

/// A pooled account's perps are inside its spot USDC, so they are taken out of spot; lending moves
/// money between slices, never the total (portfolio margin, live 2026-10-02: HYPE all lent, USDC
/// net of a loan); outcome sides and staked HYPE count apart, and a question with no price is left
/// out.
#[test]
fn each_holding_is_counted_once_in_its_own_slice() {
    let dex = DexState {
        dex: String::new(),
        state: ClearinghouseState {
            margin_summary: MarginSummary {
                account_value: dec("100"),
                total_margin_used: dec("20"),
                ..MarginSummary::default()
            },
            ..ClearinghouseState::default()
        },
    };
    let pooled = Standing {
        mode: Some(Abstraction::UnifiedAccount),
        dexes: vec![dex],
        spot: spot(vec![usdc("500")]),
        heard: None,
    };
    let apart = Standing { mode: Some(Abstraction::Disabled), ..pooled.clone() };
    let lending = Standing {
        mode: Some(Abstraction::PortfolioMargin),
        spot: Some(read(SPOT_PORTFOLIO_MARGIN)),
        ..Standing::default()
    };
    let sides = Standing {
        mode: Some(Abstraction::Disabled),
        spot: Some(read(
            r#"{"balances":[{"coin":"USDC","token":0,"total":"10","hold":"0","entryNtl":"0.0"},{"coin":"+14730","total":"63","hold":"0","entryNtl":"36.8"},{"coin":"o458","total":"5","hold":"0","entryNtl":"0.0"}]}"#,
        )),
        ..Standing::default()
    };
    let prices = Prices::new().with("#14730", dec("0.584")).with("HYPE", dec("89.785"));
    let staked = Beside { staked: dec("2.5"), ..Beside::default() };
    let worth = |perps: &str, spot: &str, lending: &str, outcomes: &str, staking: &str, margin: &str| Worth {
        perps: dec(perps),
        spot: dec(spot),
        lending: dec(lending),
        outcomes: dec(outcomes),
        staking: dec(staking),
        vaults: Dec::ZERO,
        margin: dec(margin),
    };
    for (standing, beside, expected, total) in [
        (&pooled, &Beside::default(), worth("100", "400", "0", "0", "0", "20"), "500"),
        (&apart, &Beside::default(), worth("100", "500", "0", "0", "0", "20"), "600"),
        (&lending, &Beside::default(), worth("0", "743.03464406", "110.34445601555", "0", "0", "0"), "853.37910007555"),
        (&sides, &staked, worth("0", "10", "0", "36.792", "224.4625", "0"), "271.2545"),
    ] {
        let counted = standing.worth(&prices, beside).unwrap();
        assert_eq!((counted, counted.total()), (expected, Some(dec(total))), "{:?}", standing.mode);
    }
}

/// Across accounts, a deposit one listed account holds in a vault that is itself listed is counted
/// once, as the vault; a deposit in a vault left out is the depositor's.
#[test]
fn a_deposit_in_a_listed_vault_is_counted_once() {
    let holding = |total: &str| Standing {
        mode: Some(Abstraction::Disabled),
        spot: spot(vec![usdc(total)]),
        ..Standing::default()
    };
    let (depositor, vault) = (address(STANDARD), address(UNIFIED));
    let into = Beside { vaults: vec![VaultShare { vault, equity: dec("100") }], ..Beside::default() };
    let accounts = [(depositor, holding("50"), into), (vault, holding("1000"), Beside::default())];
    let listed: Vec<(Address, &Standing, &Beside)> =
        accounts.iter().map(|(at, one, beside)| (*at, one, beside)).collect();
    let together = combined(&listed, &Prices::new()).unwrap();
    assert_eq!((together.vaults, together.total()), (Dec::ZERO, Some(dec("1050"))));
    let alone = combined(&listed[..1], &Prices::new()).unwrap();
    assert_eq!(alone.total(), Some(dec("150")));
}
