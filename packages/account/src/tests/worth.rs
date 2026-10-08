use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;
use crate::{
    fixtures::{
        PERP_STANDARD, PERP_STANDARD_XYZ, PERP_UNIFIED, PERP_UNIFIED_XYZ, PORTFOLIO_STANDARD, PORTFOLIO_UNIFIED,
        PRICES, SPOT_PORTFOLIO_MARGIN, SPOT_STANDARD, SPOT_UNIFIED, VAULTS_STANDARD, address, dec, read,
    },
    state::Abstraction,
    usable::DexState,
};

/// The recorded mids read as exact decimals, as a store holds them.
#[derive(Deserialize)]
struct Recorded {
    pairs: BTreeMap<String, String>,
    mids: BTreeMap<String, Dec>,
}

fn prices() -> Prices {
    let recorded: Recorded = read(PRICES);
    Prices::of(&recorded.pairs, &recorded.mids)
}

/// A token at its pair's mid, an outcome side at its own; a mid of nothing is no price.
#[test]
fn prices_come_from_the_pairs_and_exact_mids() {
    let mut mids: BTreeMap<String, Dec> = BTreeMap::new();
    mids.insert("@107".to_owned(), dec("83.4515"));
    mids.insert("#14730".to_owned(), dec("0.584"));
    mids.insert("@142".to_owned(), Dec::ZERO);
    let pairs: BTreeMap<String, String> =
        [("HYPE", "@107"), ("UBTC", "@142")].map(|(token, pair)| (token.to_owned(), pair.to_owned())).into();
    let prices = Prices::of(&pairs, &mids);
    assert_eq!((prices.price("HYPE"), prices.price("+14730")), (Some(dec("83.4515")), Some(dec("0.584"))));
    assert_eq!((prices.price("UBTC"), prices.price("USDC")), (None, Some(Dec::ONE)));
}

fn standing(spot: &str, main: &str, xyz: &str, mode: Abstraction) -> Standing {
    Standing {
        mode: Some(mode),
        dexes: vec![
            DexState { dex: String::new(), state: read(main) },
            DexState { dex: "xyz".to_owned(), state: read(xyz) },
        ],
        spot: Some(read(spot)),
        heard: None,
    }
}

/// The last point of `portfolio`'s `day`: the venue's own figure for what
/// the account is worth.
fn venue_says(portfolio: &str) -> Dec {
    let spans: Vec<(String, serde_json::Value)> = read(portfolio);
    let day = &spans.iter().find(|(name, _)| name == "day").unwrap().1["accountValueHistory"];
    dec(day.as_array().unwrap().last().unwrap()[1].as_str().unwrap())
}

#[test]
fn a_token_is_priced_at_its_usdc_pair_s_mid_and_usdc_at_one() {
    let prices = prices();
    assert_eq!(prices.price("USDC"), Some(Dec::ONE));
    assert_eq!(prices.price("HYPE"), Some(dec("83.4515")), "HYPE at @107");
    assert_eq!(prices.price("PURR"), Some(dec("0.11179")), "PURR at PURR/USDC");
    assert_eq!(prices.price("NOPE"), None);
    let outcome = Prices::new().with("#14730", dec("0.584"));
    assert_eq!(outcome.price("+14730"), Some(dec("0.584")), "an outcome side at the mid quoted as #");
}

/// A live unified account: the venue's own worth is its spot at mids. The
/// main dex's 549463.356168 is already inside spot USDC, so it is counted
/// once, as perps, and taken out of spot.
#[test]
fn a_unified_account_s_perps_are_inside_its_spot() {
    let standing = standing(SPOT_UNIFIED, PERP_UNIFIED, PERP_UNIFIED_XYZ, Abstraction::UnifiedAccount);
    let worth = standing.worth(&prices(), &Beside::default()).unwrap();
    assert_eq!(worth.perps, dec("549463.356168"));
    assert_eq!(worth.spot, dec("729799.2062150256144"));
    assert_eq!(worth.margin, dec("480263.227504"));
    let total = worth.total().unwrap();
    assert_eq!(total, dec("1279262.5623830256144"), "USDC, HYPE, UETH and MAX at their mids");
    let venue = venue_says(PORTFOLIO_UNIFIED);
    assert_eq!(venue, dec("1279262.5624760001"));
    assert!(venue.checked_sub(total).unwrap().abs() < dec("0.001"), "{total} against the venue's {venue}");
}

/// A live standard account: spot at mids, plus its perps beside, plus its
/// two vault deposits. The mids were read six seconds after `portfolio`.
#[test]
fn a_standard_account_s_perps_are_beside_its_spot() {
    let standing = standing(SPOT_STANDARD, PERP_STANDARD, PERP_STANDARD_XYZ, Abstraction::Default);
    let deposits: Vec<serde_json::Value> = read(VAULTS_STANDARD);
    let vaults = deposits
        .iter()
        .map(|one| VaultShare {
            vault: address(one["vaultAddress"].as_str().unwrap()),
            equity: dec(one["equity"].as_str().unwrap()),
        })
        .collect();
    let worth = standing.worth(&prices(), &Beside { vaults, ..Beside::default() }).unwrap();
    assert_eq!((worth.perps, worth.spot), (dec("463.943191"), dec("574.7059086470572")));
    assert_eq!(worth.vaults, dec("366712.374359"));
    let total = worth.total().unwrap();
    assert_eq!(total, dec("367751.0234586470572"));
    let venue = venue_says(PORTFOLIO_STANDARD);
    assert!(venue.checked_sub(total).unwrap().abs() < dec("0.1"), "{total} against the venue's {venue}");
}

/// Portfolio margin, live 2026-10-02 (atlas): HYPE is all lent and USDC net
/// of a loan. Lending moves money between slices, never the total.
#[test]
fn lending_never_moves_what_an_account_is_worth() {
    let standing = Standing {
        mode: Some(Abstraction::PortfolioMargin),
        spot: Some(read(SPOT_PORTFOLIO_MARGIN)),
        ..Standing::default()
    };
    let prices = Prices::new().with("HYPE", dec("89.785"));
    let worth = standing.worth(&prices, &Beside::default()).unwrap();
    assert_eq!(worth.lending, dec("110.34445601555"), "HYPE lent less USDC owed");
    assert_eq!(worth.spot, dec("743.03464406"));
    assert_eq!(worth.total(), Some(dec("853.37910007555")), "USDC and HYPE at their totals");
}

#[test]
fn outcome_sides_are_counted_apart_and_staked_hype_at_its_price() {
    let standing = Standing {
        mode: Some(Abstraction::Disabled),
        spot: Some(read(
            r#"{"balances":[{"coin":"USDC","token":0,"total":"10","hold":"0","entryNtl":"0.0"},{"coin":"+14730","total":"63","hold":"0","entryNtl":"36.8"},{"coin":"o458","total":"5","hold":"0","entryNtl":"0.0"}]}"#,
        )),
        ..Standing::default()
    };
    let prices = Prices::new().with("#14730", dec("0.584")).with("HYPE", dec("40"));
    let worth = standing.worth(&prices, &Beside { staked: dec("2.5"), ..Beside::default() }).unwrap();
    assert_eq!((worth.spot, worth.outcomes, worth.staking), (dec("10"), dec("36.792"), dec("100")));
    assert_eq!(worth.total(), Some(dec("146.792")), "a question with no price is left out");
}

/// hip4-tui's Portfolio across accounts: a deposit one listed account holds
/// in a vault that is itself listed is counted once, as the vault.
#[test]
fn a_deposit_in_a_listed_vault_is_counted_once() {
    let usdc = |total: &str| Standing {
        mode: Some(Abstraction::Disabled),
        spot: Some(SpotState {
            balances: vec![crate::state::SpotBalance {
                coin: "USDC".to_owned(),
                token: Some(0),
                total: dec(total),
                ..crate::state::SpotBalance::default()
            }],
            ..SpotState::default()
        }),
        ..Standing::default()
    };
    let (depositor, vault) = (address(crate::fixtures::STANDARD), address(crate::fixtures::UNIFIED));
    let into = Beside { vaults: vec![VaultShare { vault, equity: dec("100") }], ..Beside::default() };
    let accounts = [(depositor, usdc("50"), into), (vault, usdc("1000"), Beside::default())];
    let listed: Vec<(Address, &Standing, &Beside)> =
        accounts.iter().map(|(at, one, beside)| (*at, one, beside)).collect();
    let together = combined(&listed, &Prices::new()).unwrap();
    assert_eq!((together.vaults, together.total()), (Dec::ZERO, Some(dec("1050"))));
    let alone = combined(&listed[..1], &Prices::new()).unwrap();
    assert_eq!(alone.total(), Some(dec("150")), "a vault left out is the depositor's");
}

/// atlas's `holdings_of` over a live portfolio-margin account (2026-10-02):
/// its spot, `delegatorSummary`, `userVaultEquities` and
/// `borrowLendUserState`, recorded as atlas's fixtures.
#[test]
fn what_an_account_holds_beside_its_balances() {
    let standing = Standing {
        mode: Some(Abstraction::PortfolioMargin),
        spot: Some(read(SPOT_PORTFOLIO_MARGIN)),
        ..Standing::default()
    };
    let hlp = address("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303");
    let beside = Beside {
        // delegated 1.10019629, undelegated 0, pending 0.00205988.
        staked: dec("1.10225617"),
        vaults: vec![VaultShare { vault: hlp, equity: dec("10.002638") }],
        lending: vec![
            Reserve { token: 0, supplied: Dec::ZERO, borrowed: dec("188.39764406") },
            Reserve { token: 150, supplied: dec("3.32730523"), borrowed: Dec::ZERO },
        ],
        health: Some(LendingHealth { word: "atRisk".to_owned(), factor: Some(dec("0.994555261")) }),
    };
    let collateral = crate::usable::Collateral::new().with("cash", 268);
    let names: BTreeMap<u32, String> = [(0, "USDC"), (150, "HYPE"), (268, "USDT0")]
        .into_iter()
        .map(|(token, name)| (token, name.to_owned()))
        .collect();
    let prices = Prices::new().with("HYPE", dec("89.785"));
    let held = holdings(&standing, &collateral, &beside, &names, &prices);
    let hype = |amount: &str| Holding {
        coin: "HYPE".to_owned(),
        token: Some(150),
        amount: dec(amount),
        price: Some(dec("89.785")),
        place: None,
    };
    assert_eq!(held.tokens, [hype("3.32730523")], "HYPE pays for nothing on this account");
    assert_eq!(held.staked, [hype("1.10225617")]);
    assert_eq!(
        held.vaults,
        [Holding {
            coin: "USDC".to_owned(),
            token: Some(0),
            amount: dec("10.002638"),
            price: Some(Dec::ONE),
            place: Some(hlp)
        }]
    );
    assert_eq!(held.supplied, [hype("3.32730523")]);
    assert_eq!(
        held.borrowed,
        [Holding {
            coin: "USDC".to_owned(),
            token: Some(0),
            amount: dec("188.39764406"),
            price: Some(Dec::ONE),
            place: None
        }]
    );
    assert_eq!(
        held.health,
        Some(Health { factor: Some(dec("0.994555261")), risk: Risk::Near, word: "AT RISK".to_owned() })
    );
    assert_eq!(held.worth(), Some(dec("407.710808299")), "tokens, staked and vaults; lent and owed move nothing");
}

#[test]
fn lending_health_is_said_only_while_something_is_owed_or_unwell() {
    let health = |word: &str, factor: Option<&str>| {
        Beside { health: Some(LendingHealth { word: word.to_owned(), factor: factor.map(dec) }), ..Beside::default() }
            .lending_health()
    };
    assert_eq!(health("healthy", None), None);
    assert_eq!(health("healthy", Some("1.1598513145")).map(|one| one.risk), Some(Risk::Safe));
    assert_eq!(
        health("liquidatable", None).map(|one| (one.risk, one.word)),
        Some((Risk::Liquidatable, "LIQUIDATABLE".to_owned()))
    );
    assert_eq!(health("somethingNew", None).map(|one| one.risk), Some(Risk::Near), "a word not seen yet is a warning");
}
