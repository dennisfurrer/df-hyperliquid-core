//! What an account can put on each kind of market, read the way its mode says, and which balance
//! pays for an order. A pooled account's per-dex states are not a balance of their own, so they
//! are never added to spot.

use std::collections::BTreeMap;

use crate::account::holdings::keeping::Keeping;
use crate::account::holdings::spot::Risk;
use crate::account::holdings::standing::Standing;
use crate::client::USDC_INDEX;
use crate::utils::Dec;

/// What portfolio margin lends against each eligible collateral, in
/// hundredths, by the spot token's name (BTC is `UBTC` in spot), where its
/// row carries no `ltv` of its own.
pub const LTV: [(&str, i128); 2] = [("HYPE", 65), ("UBTC", 50)];

/// Each perp dex's collateral token by the dex's name (`""` the main dex), as
/// `allPerpMetas` lists `collateralToken`. A dex not listed posts USDC.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collateral(BTreeMap<String, u32>);

impl Collateral {
    /// No dex listed: every dex posts USDC.
    #[must_use]
    pub fn new() -> Collateral {
        Collateral::default()
    }

    /// Adds one dex's collateral token, by the dex's name and the token's index.
    #[must_use]
    pub fn with(mut self, dex: &str, token: u32) -> Collateral {
        self.0.insert(dex.to_owned(), token);
        self
    }

    /// The token `dex` posts margin in.
    #[must_use]
    pub fn of(&self, dex: &str) -> u32 {
        self.0.get(dex).copied().unwrap_or(USDC_INDEX)
    }

    /// Every collateral token, USDC first.
    #[must_use]
    pub fn tokens(&self) -> Vec<u32> {
        let mut tokens = vec![USDC_INDEX];
        for token in self.0.values() {
            if !tokens.contains(token) {
                tokens.push(*token);
            }
        }
        tokens[1..].sort_unstable();
        tokens
    }
}

impl FromIterator<(String, u32)> for Collateral {
    fn from_iter<I: IntoIterator<Item = (String, u32)>>(iter: I) -> Collateral {
        Collateral(iter.into_iter().collect())
    }
}

/// Where an order's money comes from: the kind of market it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface<'a> {
    /// A perp dex, `""` the main one.
    Perps {
        /// The dex's name.
        dex: &'a str,
    },
    /// A spot pair quoted in this token.
    Spot {
        /// The quote token's index.
        quote: u32,
    },
    /// HIP-4 outcomes, quoted in USDC and settled on spot.
    Outcomes,
}

impl Surface<'_> {
    /// The token an order here is paid in.
    #[must_use]
    pub fn token(&self, collateral: &Collateral) -> u32 {
        match self {
            Surface::Perps { dex } => collateral.of(dex),
            Surface::Spot { quote } => *quote,
            Surface::Outcomes => USDC_INDEX,
        }
    }
}

/// What a balance pays for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pays {
    /// One perp dex's orders, `""` the main dex.
    Perps {
        /// The dex's name.
        dex: String,
    },
    /// DEX abstraction: the main dex's USDC, for every USDC dex.
    UsdcPerps,
    /// Spot and outcome orders.
    Spot,
    /// A pooled account's one balance of a token: every market settled in it.
    Any,
}

/// One balance an order can draw on: a perp dex's margin, a spot token, or
/// a pooled account's one balance of a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    /// The token's index.
    pub token: u32,
    /// A dex's account value, or a spot token's total.
    pub total: Dec,
    /// Free to trade or move: a dex's `withdrawable`, spot's total less its hold.
    pub available: Dec,
    /// What the balance pays for.
    pub pays: Pays,
}

/// The balance an order on `surface` draws on, or `None` when the account
/// holds none that pays for it.
#[must_use]
pub fn payer<'a>(balances: &'a [Balance], surface: Surface<'_>, collateral: &Collateral) -> Option<&'a Balance> {
    let token = surface.token(collateral);
    balances.iter().find(|balance| {
        balance.token == token
            && match (&balance.pays, surface) {
                (Pays::Any, _) => true,
                (Pays::Spot, Surface::Spot { .. } | Surface::Outcomes) => true,
                (Pays::Perps { dex }, Surface::Perps { dex: wanted }) => dex == wanted,
                (Pays::UsdcPerps, Surface::Perps { .. }) => token == USDC_INDEX,
                _ => false,
            }
    })
}

/// Where usable money sits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// One perp dex's own margin.
    Perps {
        /// The dex's name, `""` the main dex.
        dex: String,
    },
    /// DEX abstraction: the main dex's USDC, for a HIP-3 USDC dex.
    NativePerps,
    /// Spot, on a standard account or for spot and outcome orders.
    Spot,
    /// A unified account's one balance in spot.
    Unified,
    /// A portfolio-margin account's balance in spot, with what it may borrow.
    PortfolioMargin,
}

/// What an account can put on a surface, and where that sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usable {
    /// How much, in `token`, borrowing included.
    pub amount: Dec,
    /// The token it is counted in.
    pub token: u32,
    /// Where it sits.
    pub from: Source,
    /// Portfolio margin: the part of `amount` that would be borrowed.
    pub borrowable: Dec,
}

impl Standing {
    /// Every balance an order can draw on, by what it pays for. Spot counts
    /// USDC and the dexes' collateral tokens: USDC always, another only when
    /// held. Any other token is a holding, not a balance.
    #[must_use]
    pub fn balances(&self, collateral: &Collateral) -> Vec<Balance> {
        let keeping = self.keeping();
        let mut out = Vec::new();
        if !keeping.pools() {
            for dex in &self.dexes {
                let token = collateral.of(&dex.dex);
                let pays = match (keeping, dex.dex.is_empty(), token == USDC_INDEX) {
                    (Keeping::Native, true, _) => Pays::UsdcPerps,
                    (Keeping::Native, false, true) => continue,
                    _ => Pays::Perps { dex: dex.dex.clone() },
                };
                let summary = &dex.state.margin_summary;
                out.push(Balance { token, total: summary.account_value, available: dex.state.withdrawable, pays });
            }
        }
        let pays = match keeping.pools() {
            true => Pays::Any,
            false => Pays::Spot,
        };
        let counted = collateral.tokens();
        let rows = self.spot.iter().flat_map(|spot| &spot.balances).filter(|row| !row.is_outcome());
        let mut usdc = false;
        for row in rows {
            let Some(token) = row.token.filter(|token| counted.contains(token)) else { continue };
            let available = row.available().unwrap_or(Dec::ZERO);
            if token != USDC_INDEX && row.total.is_zero() && available.is_zero() {
                continue;
            }
            usdc |= token == USDC_INDEX;
            out.push(Balance { token, total: row.total, available, pays: pays.clone() });
        }
        if !usdc {
            let none = Balance { token: USDC_INDEX, total: Dec::ZERO, available: Dec::ZERO, pays };
            let at = out.iter().position(|balance| balance.pays == Pays::Spot || balance.pays == Pays::Any);
            out.insert(at.unwrap_or(out.len()), none);
        }
        out
    }

    /// What this account can put on `surface`. `price` is a spot token's
    /// price in USDC by its name, which only portfolio margin's lending reads.
    /// A figure past what a `Dec` counts reads as nothing, never as more.
    #[must_use]
    pub fn usable(&self, surface: Surface<'_>, collateral: &Collateral, price: impl Fn(&str) -> Option<Dec>) -> Usable {
        let keeping = self.keeping();
        let token = surface.token(collateral);
        let balances = self.balances(collateral);
        let paying = payer(&balances, surface, collateral);
        let from = match (keeping, paying.map(|balance| &balance.pays), surface) {
            (Keeping::Unified, _, _) => Source::Unified,
            (Keeping::Portfolio, _, _) => Source::PortfolioMargin,
            (_, Some(Pays::UsdcPerps), Surface::Perps { dex }) if !dex.is_empty() => Source::NativePerps,
            (_, _, Surface::Perps { dex }) => Source::Perps { dex: dex.to_owned() },
            (_, _, Surface::Spot { .. } | Surface::Outcomes) => Source::Spot,
        };
        let free = paying.map_or(Dec::ZERO, |balance| balance.available);
        let borrowable = match (keeping, token == USDC_INDEX) {
            (Keeping::Portfolio, true) => self.borrowable(&price),
            _ => Dec::ZERO,
        };
        let amount = free.checked_add(borrowable).unwrap_or(free);
        Usable { amount, token, from, borrowable }
    }

    /// What portfolio margin would lend in USDC now: each eligible token's total at `price` and its
    /// loan-to-value (the row's own `ltv`, else [`LTV`]), less what is owed already, at most what USDC
    /// has free after maintenance where the venue says, and nothing once liquidatable. Nothing on any
    /// other mode.
    #[must_use]
    pub fn borrowable(&self, price: &impl Fn(&str) -> Option<Dec>) -> Dec {
        let Some(spot) = self.spot.as_ref().filter(|_| self.keeping() == Keeping::Portfolio) else {
            return Dec::ZERO;
        };
        if spot.risk() == Risk::Liquidatable {
            return Dec::ZERO;
        }
        let rows = || spot.balances.iter().filter(|row| !row.is_outcome());
        let room = rows()
            .filter(|row| row.token != Some(USDC_INDEX))
            .filter_map(|row| {
                let ltv = row.ltv.or_else(|| {
                    LTV.iter().find(|(coin, _)| *coin == row.coin).map(|(_, hundredths)| Dec::lit::<2>(*hundredths))
                })?;
                row.total.checked_mul(price(&row.coin)?)?.checked_mul(ltv).filter(|lent| lent.is_positive())
            })
            .fold(Dec::ZERO, |sum, lent| sum.checked_add(lent).unwrap_or(sum));
        let owed =
            rows().filter_map(|row| row.borrowed).fold(Dec::ZERO, |sum, owed| sum.checked_add(owed).unwrap_or(sum));
        let mut out = room.checked_sub(owed).unwrap_or(Dec::ZERO).max(Dec::ZERO);
        if let Some(after) = spot.available_after_maintenance(USDC_INDEX) {
            out = out.min(after.max(Dec::ZERO));
        }
        out
    }
}

#[cfg(test)]
#[path = "tests/usable.rs"]
mod tests;
