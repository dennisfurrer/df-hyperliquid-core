//! What an account is worth, counted once, and what it holds beside the
//! balances its orders draw on.
//!
//! The venue's own sum (`portfolio`'s account value), checked live on
//! 2026-10-08 against a unified and a standard account, is spot at mids,
//! plus the perp dexes only when they are not already inside spot, plus vault
//! equity and staked HYPE. A unified or portfolio-margin account holds its
//! perp equity, on every dex, inside its spot USDC: its perps are counted as
//! perps and taken out of spot. A standard account's are beside it.
//!
//! Lending moves money between slices, never the total: a lent amount is
//! part of the token's total, and a borrowed one is already netted out of
//! the balance it was drawn into (atlas, checked live 2026-10-02).
//!
//! Staking, vaults and lending are read by hl-stake, hl-vault and hl-lend;
//! their figures come in here as [`Beside`].

use std::collections::{BTreeMap, BTreeSet};

use hl_core::{Address, Dec};

use crate::{
    state::{Risk, SpotState},
    usable::{Collateral, Standing, USDC},
};

/// What the HYPE staked is staked in.
const STAKED: &str = "HYPE";

/// Prices in USDC by the name an account's rows carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prices(BTreeMap<String, Dec>);

impl Prices {
    #[must_use]
    pub fn new() -> Prices {
        Prices::default()
    }

    /// One price, by a token's name (`HYPE`) or a mid's (`#14730`).
    #[must_use]
    pub fn with(mut self, name: &str, price: Dec) -> Prices {
        self.0.insert(name.to_owned(), price);
        self
    }

    /// From each token's USDC pair (`HYPE` is `@107`, as `spotMeta` lists the
    /// pairs) and `allMids`: a token at its pair's mid, and an outcome side's
    /// mid (`#14730`) as it is quoted. A mid that does not read is left out.
    #[must_use]
    pub fn of(pairs: &BTreeMap<String, String>, mids: &BTreeMap<String, String>) -> Prices {
        let read = |mid: &String| Dec::parse(mid).ok().filter(|price| price.is_positive());
        let mut prices: BTreeMap<String, Dec> = mids
            .iter()
            .filter(|(name, _)| name.starts_with('#'))
            .filter_map(|(name, mid)| Some((name.clone(), read(mid)?)))
            .collect();
        for (token, pair) in pairs {
            if let Some(price) = mids.get(pair).and_then(read) {
                prices.insert(token.clone(), price);
            }
        }
        Prices(prices)
    }

    /// A row's price: one for USDC, an outcome side `+N` at the mid quoted as
    /// `#N`, any other token at its own.
    #[must_use]
    pub fn price(&self, coin: &str) -> Option<Dec> {
        if coin == "USDC" {
            return Some(Dec::ONE);
        }
        match coin.strip_prefix('+') {
            Some(side) => self.0.get(&format!("#{side}")).copied(),
            None => self.0.get(coin).copied(),
        }
    }
}

/// What an account holds that other crates read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Beside {
    /// HYPE staked: delegated, not delegated, and on its way out
    /// (`delegatorSummary`, hl-stake).
    pub staked: Dec,
    /// Each vault deposit (`userVaultEquities`, hl-vault).
    pub vaults: Vec<VaultShare>,
    /// Each token lent and borrowed (`borrowLendUserState`, hl-lend).
    pub lending: Vec<Reserve>,
    pub health: Option<LendingHealth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultShare {
    pub vault: Address,
    /// In USDC.
    pub equity: Dec,
}

/// One token's lending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reserve {
    pub token: u32,
    pub supplied: Dec,
    pub borrowed: Dec,
}

/// The lending health as `borrowLendUserState` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LendingHealth {
    /// `healthy`, `atRisk`, `liquidatable`, as the venue words it.
    pub word: String,
    pub factor: Option<Dec>,
}

impl Beside {
    /// The health worth showing: while something is borrowed (a factor), or
    /// in any word but `healthy`. A word not seen yet is a warning.
    #[must_use]
    pub fn lending_health(&self) -> Option<Health> {
        let health = self.health.as_ref()?;
        if health.factor.is_none() && health.word == "healthy" {
            return None;
        }
        let risk = match health.word.as_str() {
            "healthy" => Risk::Safe,
            "liquidatable" => Risk::Liquidatable,
            _ => Risk::Near,
        };
        Some(Health { factor: health.factor, risk, word: capitals(&health.word) })
    }
}

/// `atRisk` as `AT RISK`.
fn capitals(camel: &str) -> String {
    let mut out = String::new();
    for letter in camel.chars() {
        if letter.is_ascii_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.push(letter.to_ascii_uppercase());
    }
    out
}

/// What an account is worth, by where it sits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Worth {
    /// Every perp dex's account value.
    pub perps: Dec,
    /// Spot at mids, less what is lent, and less the perps on a pooled
    /// account, whose perps are inside it.
    pub spot: Dec,
    /// Lent less borrowed.
    pub lending: Dec,
    /// HIP-4 sides at their mids.
    pub outcomes: Dec,
    pub staking: Dec,
    pub vaults: Dec,
    /// Margin in use across the dexes; part of `perps`, not added.
    pub margin: Dec,
}

impl Worth {
    /// Everything but `margin`. `None` only past what a `Dec` counts.
    #[must_use]
    pub fn total(&self) -> Option<Dec> {
        [self.spot, self.lending, self.outcomes, self.staking, self.vaults]
            .into_iter()
            .try_fold(self.perps, Dec::checked_add)
    }

    #[must_use]
    pub fn plus(&self, other: &Worth) -> Option<Worth> {
        Some(Worth {
            perps: self.perps.checked_add(other.perps)?,
            spot: self.spot.checked_add(other.spot)?,
            lending: self.lending.checked_add(other.lending)?,
            outcomes: self.outcomes.checked_add(other.outcomes)?,
            staking: self.staking.checked_add(other.staking)?,
            vaults: self.vaults.checked_add(other.vaults)?,
            margin: self.margin.checked_add(other.margin)?,
        })
    }
}

impl Standing {
    /// What this account is worth at `prices`, counted once. A row with no
    /// price is left out. `None` only past what a `Dec` counts.
    #[must_use]
    pub fn worth(&self, prices: &Prices, beside: &Beside) -> Option<Worth> {
        let mut worth = Worth::default();
        for dex in &self.dexes {
            worth.perps = worth.perps.checked_add(dex.state.margin_summary.account_value)?;
            worth.margin = worth.margin.checked_add(dex.state.margin_summary.total_margin_used)?;
        }
        let mut spot = Dec::ZERO;
        for row in self.spot.iter().flat_map(|spot| &spot.balances) {
            let Some(price) = prices.price(&row.coin) else { continue };
            let value = row.total.checked_mul(price)?;
            if row.is_outcome() {
                worth.outcomes = worth.outcomes.checked_add(value)?;
                continue;
            }
            spot = spot.checked_add(value)?;
            let lent = row.supplied.unwrap_or(Dec::ZERO).checked_mul(price)?;
            let owed = row.borrowed.unwrap_or(Dec::ZERO).checked_mul(price)?;
            worth.lending = worth.lending.checked_add(lent)?.checked_sub(owed)?;
        }
        spot = spot.checked_sub(worth.lending)?;
        if self.pools() {
            spot = spot.checked_sub(worth.perps)?;
        }
        worth.spot = spot;
        worth.staking = match beside.staked.is_zero() {
            true => Dec::ZERO,
            false => beside.staked.checked_mul(prices.price(STAKED).unwrap_or(Dec::ZERO))?,
        };
        worth.vaults = beside.vaults.iter().try_fold(Dec::ZERO, |sum, share| sum.checked_add(share.equity))?;
        Some(worth)
    }
}

/// Several accounts summed, each counted once: a deposit one of them holds
/// in a vault that is itself one of them is counted as the vault, and a
/// deposit in a vault left out as the depositor's.
#[must_use]
pub fn combined(accounts: &[(Address, &Standing, &Beside)], prices: &Prices) -> Option<Worth> {
    let listed: BTreeSet<Address> = accounts.iter().map(|(address, _, _)| *address).collect();
    let mut worth = Worth::default();
    for (_, standing, beside) in accounts {
        worth = worth.plus(&standing.worth(prices, beside)?)?;
        for share in beside.vaults.iter().filter(|share| listed.contains(&share.vault)) {
            worth.vaults = worth.vaults.checked_sub(share.equity)?;
        }
    }
    Some(worth)
}

/// An amount of one asset, and its price in USDC where one is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    pub coin: String,
    pub token: Option<u32>,
    pub amount: Dec,
    pub price: Option<Dec>,
    /// Where it is held apart from the account: a vault's address.
    pub place: Option<Address>,
}

/// How safe what is borrowed stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub factor: Option<Dec>,
    pub risk: Risk,
    /// The venue's word in capitals: `AT RISK`.
    pub word: String,
}

/// What an account holds beside the balances its orders draw on
/// ([`Standing::balances`]).
///
/// What it is worth adds `tokens`, `staked` and `vaults` to the balances.
/// What is lent stays in the balance or token that holds it, and what is
/// borrowed is already netted out of the balance it was drawn into: neither
/// changes the worth.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Holdings {
    /// Spot tokens no balance counts: not USDC, nor a dex's collateral.
    pub tokens: Vec<Holding>,
    pub staked: Vec<Holding>,
    pub vaults: Vec<Holding>,
    pub supplied: Vec<Holding>,
    pub borrowed: Vec<Holding>,
    pub health: Option<Health>,
}

impl Holdings {
    /// Tokens, staked and vaults at their prices; one with no price is left
    /// out. `None` only past what a `Dec` counts.
    #[must_use]
    pub fn worth(&self) -> Option<Dec> {
        self.tokens.iter().chain(&self.staked).chain(&self.vaults).try_fold(Dec::ZERO, |sum, held| match held.price {
            Some(price) => sum.checked_add(held.amount.checked_mul(price)?),
            None => Some(sum),
        })
    }
}

/// atlas's `holdings_of`: what `standing` holds beside its balances, each
/// priced. `names` is each token's name by index, as `spotMeta` lists them.
#[must_use]
pub fn holdings(
    standing: &Standing,
    collateral: &Collateral,
    beside: &Beside,
    names: &BTreeMap<u32, String>,
    prices: &Prices,
) -> Holdings {
    let counted = collateral.tokens();
    let held = |coin: &str, token: Option<u32>, amount: Dec| Holding {
        coin: coin.to_owned(),
        token,
        amount,
        price: prices.price(coin),
        place: None,
    };
    let tokens = standing
        .spot
        .iter()
        .flat_map(|spot: &SpotState| &spot.balances)
        .filter(|row| !row.is_outcome() && !row.total.is_zero())
        .filter(|row| row.token.is_some_and(|token| !counted.contains(&token)))
        .map(|row| held(&row.coin, row.token, row.total))
        .collect();
    let index_of = |name: &str| names.iter().find(|(_, known)| known.as_str() == name).map(|(index, _)| *index);
    let staked = match beside.staked.is_positive() {
        true => vec![held(STAKED, index_of(STAKED), beside.staked)],
        false => Vec::new(),
    };
    let vaults = beside
        .vaults
        .iter()
        .filter(|share| share.equity.is_positive())
        .map(|share| Holding { place: Some(share.vault), ..held("USDC", Some(USDC), share.equity) })
        .collect();
    let name = |token: u32| names.get(&token).cloned().unwrap_or_else(|| format!("#{token}"));
    let mut supplied = Vec::new();
    let mut borrowed = Vec::new();
    for reserve in &beside.lending {
        if reserve.supplied.is_positive() {
            supplied.push(held(&name(reserve.token), Some(reserve.token), reserve.supplied));
        }
        if reserve.borrowed.is_positive() {
            borrowed.push(held(&name(reserve.token), Some(reserve.token), reserve.borrowed));
        }
    }
    Holdings { tokens, staked, vaults, supplied, borrowed, health: beside.lending_health() }
}

#[cfg(test)]
#[path = "tests/worth.rs"]
mod tests;
