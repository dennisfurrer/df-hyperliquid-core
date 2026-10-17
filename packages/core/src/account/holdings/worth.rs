//! What an account is worth, counted once, at prices from the mids: perps, spot, lending, outcome
//! sides, staking and vaults. A pooled account holds its perp equity inside spot USDC, so its perps
//! are taken out of spot; lending moves money between slices, never the total.

use std::collections::{BTreeMap, BTreeSet};

use crate::account::holdings::standing::Standing;
use crate::utils::{Address, Dec};

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
    /// pairs) and the first dex's mids, exact (`allMids`, read or sent): a
    /// token at its pair's mid, and an outcome side's mid (`#14730`) as it is
    /// quoted. A mid of nothing is no price.
    #[must_use]
    pub fn of(pairs: &BTreeMap<String, String>, mids: &BTreeMap<String, Dec>) -> Prices {
        let priced = |mid: &Dec| Some(*mid).filter(|price| price.is_positive());
        let mut prices: BTreeMap<String, Dec> = mids
            .iter()
            .filter(|(name, _)| name.starts_with('#'))
            .filter_map(|(name, mid)| Some((name.clone(), priced(mid)?)))
            .collect();
        for (token, pair) in pairs {
            if let Some(price) = mids.get(pair).and_then(priced) {
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

/// What an account holds in staking and vaults, as [`crate::earn`] reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Beside {
    /// HYPE staked: delegated, not delegated, and on its way out (`delegatorSummary`).
    pub staked: Dec,
    /// Each vault deposit (`userVaultEquities`).
    pub vaults: Vec<VaultShare>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultShare {
    pub vault: Address,
    /// In USDC.
    pub equity: Dec,
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
        Dec::checked_sum([self.perps, self.spot, self.lending, self.outcomes, self.staking, self.vaults])
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
        worth.vaults = Dec::checked_sum(beside.vaults.iter().map(|share| share.equity))?;
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

#[cfg(test)]
#[path = "tests/worth.rs"]
mod tests;
