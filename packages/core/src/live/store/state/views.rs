//! The store's views, pure reads of its state: each market's book, mids, context, prints and
//! candles, and each account's positions, orders, what it can put on an order and what it is worth.

use std::collections::BTreeMap;

use crate::account::{Beside, Collateral, FrontendOrder, Position, Prices, Standing, Surface, Usable, Worth, combined};
use crate::client::USDC_INDEX;
use crate::live::store::{Account, Context, Stamped, Store};
use crate::markets::{Book, Candle, Interval, Level, Trade, split_coin};
use crate::utils::{Address, Dec};

impl Store {
    /// A market's book, as last told.
    #[must_use]
    pub fn book(&self, coin: &str) -> Option<&Stamped<Book>> {
        self.markets.books.get(coin)
    }

    /// The best bid and the best ask of the book held.
    #[must_use]
    pub fn best(&self, coin: &str) -> (Option<Level>, Option<Level>) {
        self.book(coin).map_or((None, None), |book| (book.bids.first().copied(), book.asks.first().copied()))
    }

    /// A coin's mid: halfway between the touch where a book with both sides is held (sent on
    /// every change), else what `allMids` said, else the context's.
    #[must_use]
    pub fn mid(&self, coin: &str) -> Option<Dec> {
        if let Some(mid) = self.book(coin).and_then(|book| book.mid()) {
            return Some(mid);
        }
        let dex = split_coin(coin).map_or("", |(dex, _)| dex);
        if let Some(mid) = self.markets.mids.get(dex).and_then(|mids| mids.get(coin)) {
            return Some(*mid);
        }
        self.context(coin).and_then(|ctx| ctx.mid())
    }

    /// One perp dex's mids, `""` the first.
    #[must_use]
    pub fn mids(&self, dex: &str) -> Option<&Stamped<BTreeMap<String, Dec>>> {
        self.markets.mids.get(dex)
    }

    /// A market's context.
    #[must_use]
    pub fn context(&self, coin: &str) -> Option<Stamped<Context<'_>>> {
        if let Some(ctx) = self.markets.perps.get(coin) {
            return Some(Stamped { value: Context::Perp(&ctx.value), heard: ctx.heard, changed: ctx.changed });
        }
        let ctx = self.markets.spots.get(coin)?;
        Some(Stamped { value: Context::Spot(&ctx.value), heard: ctx.heard, changed: ctx.changed })
    }

    /// A market's prints, newest first.
    #[must_use]
    pub fn trades(&self, coin: &str) -> Option<&Stamped<Vec<Trade>>> {
        self.markets.tapes.get(coin)
    }

    /// A market's candles at `interval`, oldest first, the last one perhaps still forming.
    #[must_use]
    pub fn candles(&self, coin: &str, interval: Interval) -> Option<&Stamped<Vec<Candle>>> {
        self.markets.candles.get(&(coin.to_owned(), interval))
    }

    /// Each perp dex's collateral token, as the metas read say.
    #[must_use]
    pub const fn collateral(&self) -> &Collateral {
        &self.markets.collateral
    }

    #[must_use]
    pub fn account(&self, user: Address) -> Option<&Account> {
        self.accounts.get(&user)
    }

    /// Every open position of `user`, dex by dex.
    #[must_use]
    pub fn positions(&self, user: Address) -> Vec<&Position> {
        self.account(user).map_or_else(Vec::new, |account| account.positions().collect())
    }

    /// Every resting order of `user`, dex by dex.
    #[must_use]
    pub fn orders(&self, user: Address) -> Vec<&FrontendOrder> {
        self.account(user).map_or_else(Vec::new, |account| account.all_orders().collect())
    }

    /// The kind of market `coin` is, and so what an order on it draws on: an outcome side, a
    /// spot pair (its quote as spot meta lists it; `None` until spot meta is read), or a perp on
    /// its dex.
    #[must_use]
    pub fn surface<'a>(&self, coin: &'a str) -> Option<Surface<'a>> {
        if coin.starts_with(['#', '+']) {
            return Some(Surface::Outcomes);
        }
        if coin.starts_with('@') || coin.contains('/') {
            let pair = self.markets.spot_meta.as_ref()?.pair_named(coin)?;
            return Some(Surface::Spot { quote: pair.quote() });
        }
        Some(Surface::Perps { dex: split_coin(coin).map_or("", |(dex, _)| dex) })
    }

    /// What `user` can put on an order in `coin`, read the way its mode says (portfolio margin's
    /// lending priced at [`Store::price`]). `None` until the account's state has been told.
    #[must_use]
    pub fn usable(&self, user: Address, coin: &str) -> Option<Usable> {
        let surface = self.surface(coin)?;
        let account = self.account(user).filter(|account| account.spot.is_some() || !account.dexes.is_empty())?;
        Some(account.standing().usable(surface, &self.markets.collateral, |name| self.price(name)))
    }

    /// What one of a spot balance's rows is worth in USDC, by the name the row carries: USDC at
    /// one, a token at the mid of its pair with USDC (spot meta names the pair), an outcome side
    /// `+N` at the mid quoted as `#N`. A mid of nothing is no price.
    #[must_use]
    pub fn price(&self, name: &str) -> Option<Dec> {
        if name == "USDC" {
            return Some(Dec::ONE);
        }
        let mids = self.markets.mids.get("")?;
        let mid = |coin: &str| mids.get(coin).copied().filter(|mid| mid.is_positive());
        if let Some(side) = name.strip_prefix('+') {
            return mid(&format!("#{side}"));
        }
        let meta = self.markets.spot_meta.as_ref()?;
        mid(&meta.pair_of(meta.token_named(name)?.index, USDC_INDEX)?.name)
    }

    /// Every price [`Store::price`] knows, as [`Prices`]: each token's first pair with USDC (as
    /// spot meta's `pair_of` finds it) and the first dex's mids.
    #[must_use]
    pub fn prices(&self) -> Prices {
        let Some(mids) = self.markets.mids.get("") else { return Prices::new() };
        let mut pairs: BTreeMap<String, String> = BTreeMap::new();
        if let Some(meta) = self.markets.spot_meta.as_ref() {
            let names: BTreeMap<u32, &str> =
                meta.tokens.iter().map(|token| (token.index, token.name.as_str())).collect();
            for pair in meta.universe.iter().filter(|pair| pair.quote() == USDC_INDEX) {
                if let Some(name) = names.get(&pair.base()) {
                    pairs.entry((*name).to_owned()).or_insert_with(|| pair.name.clone());
                }
            }
        }
        Prices::of(&pairs, mids)
    }

    /// What `user` is worth, counted once. `beside` is what is read of it elsewhere: staking,
    /// vaults, lending.
    #[must_use]
    pub fn worth(&self, user: Address, beside: &Beside) -> Option<Worth> {
        self.account(user)?.standing().worth(&self.prices(), beside)
    }

    /// Several accounts summed, each counted once: a deposit one of them holds in a vault that
    /// is itself one of them is counted as the vault. An account not held is left out.
    #[must_use]
    pub fn combined(&self, accounts: &[(Address, Beside)]) -> Option<Worth> {
        let held: Vec<(Address, &Standing, &Beside)> = accounts
            .iter()
            .filter_map(|(user, beside)| Some((*user, self.account(*user)?.standing(), beside)))
            .collect();
        combined(&held, &self.prices())
    }
}

#[cfg(test)]
#[path = "tests/views.rs"]
mod tests;
