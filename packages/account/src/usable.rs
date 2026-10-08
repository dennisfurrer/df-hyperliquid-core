//! What an account can put on each kind of market, read the way its mode
//! says (Hyperliquid's docs, account abstraction modes and portfolio margin,
//! as hip4-tui and atlas read them on 2026-09-30):
//!
//! | mode | what an order can use |
//! |---|---|
//! | standard (`disabled`, `default`) | on a perp dex, that dex's `withdrawable`; on spot and outcomes, the quote token in spot less what orders hold |
//! | unified | one balance per asset, held in spot, for every dex and spot alike; a USDT dex draws on USDT |
//! | portfolio margin | as unified, plus what it would lend in USDC against HYPE and BTC |
//! | DEX abstraction | the main dex's USDC, for every USDC dex |
//!
//! A pooled account's per-dex states are not a balance of their own (the
//! docs: "individual perp dex user states are not meaningful"), so they are
//! never added to spot. Everything here is a pure function over the states
//! the caller read: no request, no clock.

use std::collections::BTreeMap;

use hl_core::{Address, Dec, USDC_INDEX};

use crate::state::{Abstraction, PerpState, SpotState};

/// USDC's token index: the main dex's collateral, and what portfolio margin lends.
pub const USDC: u32 = USDC_INDEX;

/// The users the venue lets one IP's socket follow.
pub const MOST_USERS: usize = 10;

/// What portfolio margin lends against each eligible collateral, in
/// hundredths, by the spot token's name (BTC is `UBTC` in spot), where its
/// row carries no `ltv` of its own.
pub const LTV: [(&str, i128); 2] = [("HYPE", 65), ("UBTC", 50)];

/// Each perp dex's collateral token by the dex's name (`""` the main dex), as
/// `allPerpMetas` lists `collateralToken`. A dex not listed posts USDC.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collateral(BTreeMap<String, u32>);

impl Collateral {
    #[must_use]
    pub fn new() -> Collateral {
        Collateral::default()
    }

    #[must_use]
    pub fn with(mut self, dex: &str, token: u32) -> Collateral {
        self.0.insert(dex.to_owned(), token);
        self
    }

    /// The token `dex` posts margin in.
    #[must_use]
    pub fn of(&self, dex: &str) -> u32 {
        self.0.get(dex).copied().unwrap_or(USDC)
    }

    /// Every collateral token, USDC first.
    #[must_use]
    pub fn tokens(&self) -> Vec<u32> {
        let mut tokens = vec![USDC];
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
    Perps { dex: &'a str },
    /// A spot pair quoted in this token.
    Spot { quote: u32 },
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
            Surface::Outcomes => USDC,
        }
    }
}

/// How an account keeps its balances, from its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keeping {
    /// Standard, a mode this build does not know, or none read: each perp
    /// dex and spot keep their own.
    Apart,
    /// DEX abstraction: as standard, but every USDC dex draws on the main dex.
    Native,
    /// Unified: one balance per asset, held in spot.
    Unified,
    /// Portfolio margin: unified, and an order may borrow.
    Portfolio,
}

impl Keeping {
    /// From the mode read, or, with none read, from the spot answer's own
    /// portfolio-margin flag.
    #[must_use]
    pub fn of(mode: Option<&Abstraction>, spot: Option<&SpotState>) -> Keeping {
        match mode {
            Some(Abstraction::UnifiedAccount) => Keeping::Unified,
            Some(Abstraction::PortfolioMargin) => Keeping::Portfolio,
            Some(Abstraction::DexAbstraction) => Keeping::Native,
            Some(_) => Keeping::Apart,
            None if spot.is_some_and(|spot| spot.portfolio_margin_enabled) => Keeping::Portfolio,
            None => Keeping::Apart,
        }
    }

    /// Whether perps and spot draw on one balance.
    #[must_use]
    pub const fn pools(self) -> bool {
        matches!(self, Keeping::Unified | Keeping::Portfolio)
    }

    /// The balance a movement of USDC spends.
    #[must_use]
    pub const fn draws(self, movement: Move) -> Draws {
        if self.pools() {
            return Draws::Pooled;
        }
        match movement {
            Move::ClassTransfer { to_perp: true } => Draws::Spot,
            Move::ClassTransfer { to_perp: false } | Move::Send | Move::Withdraw => Draws::Perp,
        }
    }

    /// Why a movement is refused for this account, when it is: nothing moves
    /// between the perps and spot of an account that keeps one balance.
    #[must_use]
    pub fn refuses(self, movement: Move) -> Option<String> {
        let account = match (self, movement) {
            (Keeping::Unified, Move::ClassTransfer { .. }) => "a unified account",
            (Keeping::Portfolio, Move::ClassTransfer { .. }) => "a portfolio margin account",
            _ => return None,
        };
        Some(format!("{account} holds USDC in one balance, so there is nothing to move between perps and spot"))
    }
}

/// A movement of the account's USDC (hip4-tui's move form).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Move {
    /// `usdClassTransfer`: between the account's own perps and spot.
    ClassTransfer { to_perp: bool },
    /// `usdSend`: to another account on the venue.
    Send,
    /// `withdraw3`: off the venue, to Arbitrum.
    Withdraw,
}

/// The balance a movement spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Draws {
    /// Spot USDC less its hold.
    Spot,
    /// What the main perp dex can move out.
    Perp,
    /// A unified or portfolio-margin account's one balance, held in spot.
    Pooled,
}

/// What a balance pays for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pays {
    /// One perp dex's orders, `""` the main dex.
    Perps { dex: String },
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
    pub token: u32,
    /// A dex's account value, or a spot token's total.
    pub total: Dec,
    /// Free to trade or move: a dex's `withdrawable`, spot's total less its hold.
    pub available: Dec,
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
                (Pays::UsdcPerps, Surface::Perps { .. }) => token == USDC,
                _ => false,
            }
    })
}

/// Where usable money sits, in a page's words ([`Source::words`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    /// One perp dex's own margin.
    Perps {
        dex: String,
    },
    /// DEX abstraction: the main dex's USDC, for a HIP-3 USDC dex.
    NativePerps,
    Spot,
    Unified,
    PortfolioMargin,
}

impl Source {
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Source::Perps { dex } if dex.is_empty() => "perps".to_owned(),
            Source::Perps { dex } => format!("perps on {dex}"),
            Source::NativePerps => "perps, the main dex's USDC".to_owned(),
            Source::Spot => "spot".to_owned(),
            Source::Unified => "unified, in spot".to_owned(),
            Source::PortfolioMargin => "portfolio margin".to_owned(),
        }
    }
}

/// What an account can put on a surface, and where that sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usable {
    pub amount: Dec,
    /// The token it is counted in.
    pub token: u32,
    pub from: Source,
    /// Portfolio margin: the part of `amount` that would be borrowed.
    pub borrowable: Dec,
}

/// One perp dex's clearinghouse, by the dex's name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DexState {
    /// `""` for the main dex.
    pub dex: String,
    pub state: PerpState,
}

/// One account as last read: its mode, each dex it was read on, and spot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    /// `None` until a read or the socket says.
    pub mode: Option<Abstraction>,
    pub dexes: Vec<DexState>,
    pub spot: Option<SpotState>,
    /// When it was last told anything, in milliseconds.
    pub heard: Option<u64>,
}

impl Standing {
    #[must_use]
    pub fn dex(&self, dex: &str) -> Option<&PerpState> {
        self.dexes.iter().find(|state| state.dex == dex).map(|state| &state.state)
    }

    #[must_use]
    pub fn keeping(&self) -> Keeping {
        Keeping::of(self.mode.as_ref(), self.spot.as_ref())
    }

    /// Whether perp margin is drawn from spot, so a dex's value is already
    /// inside the spot balances.
    #[must_use]
    pub fn pools(&self) -> bool {
        self.keeping().pools()
    }

    /// A spot token less what orders hold of it, never below nothing.
    #[must_use]
    pub fn free(&self, token: u32) -> Dec {
        self.spot.as_ref().map_or(Dec::ZERO, |spot| spot.free(token))
    }

    /// Every balance an order can draw on, by what it pays for. Spot counts
    /// USDC and the dexes' collateral tokens: USDC always, another only when
    /// held. Any other token is a holding, not a balance
    /// ([`crate::worth::holdings`]).
    #[must_use]
    pub fn balances(&self, collateral: &Collateral) -> Vec<Balance> {
        let keeping = self.keeping();
        let mut out = Vec::new();
        if !keeping.pools() {
            for dex in &self.dexes {
                let token = collateral.of(&dex.dex);
                let pays = match (keeping, dex.dex.is_empty(), token == USDC) {
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
            if token != USDC && row.total.is_zero() && available.is_zero() {
                continue;
            }
            usdc |= token == USDC;
            out.push(Balance { token, total: row.total, available, pays: pays.clone() });
        }
        if !usdc {
            let none = Balance { token: USDC, total: Dec::ZERO, available: Dec::ZERO, pays };
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
        let borrowable = match (keeping, token == USDC) {
            (Keeping::Portfolio, true) => self.borrowable(&price),
            _ => Dec::ZERO,
        };
        let amount = free.checked_add(borrowable).unwrap_or(free);
        Usable { amount, token, from, borrowable }
    }

    /// What portfolio margin would lend in USDC now: each eligible token's
    /// total at `price` and its loan-to-value (the row's own `ltv`, else
    /// [`LTV`]), less what is owed already, at most what USDC has free after
    /// maintenance where the venue says, and nothing once liquidatable
    /// (atlas, `use-buying-power.ts`). Nothing on any other mode.
    #[must_use]
    pub fn borrowable(&self, price: &impl Fn(&str) -> Option<Dec>) -> Dec {
        let Some(spot) = self.spot.as_ref().filter(|_| self.keeping() == Keeping::Portfolio) else {
            return Dec::ZERO;
        };
        if spot.risk() == crate::state::Risk::Liquidatable {
            return Dec::ZERO;
        }
        let rows = || spot.balances.iter().filter(|row| !row.is_outcome());
        let room = rows()
            .filter(|row| row.token != Some(USDC))
            .filter_map(|row| {
                let ltv = row.ltv.or_else(|| {
                    LTV.iter().find(|(coin, _)| *coin == row.coin).and_then(|(_, hundredths)| Dec::new(*hundredths, 2))
                })?;
                row.total.checked_mul(price(&row.coin)?)?.checked_mul(ltv).filter(|lent| lent.is_positive())
            })
            .fold(Dec::ZERO, |sum, lent| sum.checked_add(lent).unwrap_or(sum));
        let owed =
            rows().filter_map(|row| row.borrowed).fold(Dec::ZERO, |sum, owed| sum.checked_add(owed).unwrap_or(sum));
        let mut out = room.checked_sub(owed).unwrap_or(Dec::ZERO).max(Dec::ZERO);
        if let Some(after) = spot.available_after_maintenance(USDC) {
            out = out.min(after.max(Dec::ZERO));
        }
        out
    }
}

/// Every account followed at one moment: the connected one and its
/// subaccounts, each as last told, at most [`MOST_USERS`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standings {
    by: BTreeMap<Address, Standing>,
    /// The connected account first.
    followed: Vec<Address>,
}

impl Standings {
    #[must_use]
    pub fn of(&self, address: Address) -> Option<&Standing> {
        self.by.get(&address)
    }

    #[must_use]
    pub fn followed(&self) -> &[Address] {
        &self.followed
    }

    /// Follows these users, the first kept first, at most [`MOST_USERS`]. One
    /// no longer followed is let go. Says whether anything changed.
    pub fn follow(&mut self, users: &[Address]) -> bool {
        let mut wanted: Vec<Address> = Vec::new();
        for user in users {
            if !wanted.contains(user) && wanted.len() < MOST_USERS {
                wanted.push(*user);
            }
        }
        if wanted == self.followed {
            return false;
        }
        self.by.retain(|address, _| wanted.contains(address));
        self.followed = wanted;
        true
    }

    /// Changes what is known of `user`, heard at `now` milliseconds. A user
    /// not followed is not kept; says whether it was.
    pub fn accept(&mut self, user: Address, now: u64, change: impl FnOnce(&mut Standing)) -> bool {
        if !self.followed.contains(&user) {
            return false;
        }
        let standing = self.by.entry(user).or_default();
        change(standing);
        standing.heard = Some(now);
        true
    }

    /// The mode, when something said one; saying none keeps what was known.
    pub fn accept_mode(&mut self, user: Address, mode: Option<Abstraction>, now: u64) {
        if let Some(mode) = mode {
            self.accept(user, now, |standing| standing.mode = Some(mode));
        }
    }
}

#[cfg(test)]
#[path = "tests/usable.rs"]
mod tests;
