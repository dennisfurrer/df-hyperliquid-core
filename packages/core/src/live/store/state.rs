//! The store: one network's live state, the interests that keep it live, and its views.

use std::collections::BTreeMap;

use crate::account::ClearinghouseState;
use crate::account::FrontendOrder;
use crate::account::positions::Position;
use crate::account::{Abstraction, Beside, Collateral, Prices, Standing, Surface, Usable, Worth, combined};
use crate::client::{Network, USDC_INDEX};
use crate::live::store::account::{Account, Keep};
use crate::live::store::convert;
use crate::live::store::interest::{Changes, Handle, Interest, Interests};
use crate::live::store::market::{BARS, Context, Markets, fold_bar, fold_prints};
use crate::live::store::stamp::Stamped;
use crate::live::{Book, Candle, Frame, Interval, LedgerUpdate, Level, Limits, Refused, Session, Subscription, Trade};
use crate::markets::Mids;
use crate::markets::perps::{MetaAndAssetCtxs, PerpCtx, UserFunding};
use crate::markets::split_coin;
use crate::markets::spot::{SpotCtx, SpotMeta, SpotMetaAndAssetCtxs};
use crate::utils::{Address, Dec};

/// A typed `/info` answer for the store to fold.
#[derive(Debug, Clone, Copy)]
pub enum Read<'a> {
    /// `l2Book`, ungrouped.
    Book(&'a Book),
    /// `recentTrades`, newest first.
    Trades(&'a [Trade]),
    /// `candleSnapshot`, oldest first: what the socket already sent of the same candles stays.
    Candles(&'a [crate::markets::Candle]),
    /// `allMids` of one perp dex, `""` the first.
    Mids { dex: &'a str, mids: &'a Mids },
    /// `metaAndAssetCtxs`: the first dex's universe, collateral and contexts.
    Perps(&'a MetaAndAssetCtxs),
    /// `metaAndAssetCtxs` with a HIP-3 `dex`.
    Dex { dex: &'a str, read: &'a MetaAndAssetCtxs },
    /// `perpDexs` with `allPerpMetas`: every dex's universe and collateral.
    Dexes(&'a crate::markets::dexes::Dexes),
    /// `spotMetaAndAssetCtxs`.
    Spot(&'a SpotMetaAndAssetCtxs),
    /// `spotMeta`: the tokens and pairs that price spot balances.
    SpotMeta(&'a SpotMeta),
    /// `clearinghouseState` of one dex, at the account's own address.
    Clearinghouse { user: Address, dex: &'a str, state: &'a ClearinghouseState },
    /// `spotClearinghouseState`.
    SpotState { user: Address, state: &'a crate::account::SpotState },
    /// `userAbstraction`.
    Mode { user: Address, mode: &'a Abstraction },
    /// `frontendOpenOrders` of one dex.
    Orders { user: Address, dex: &'a str, orders: &'a [FrontendOrder] },
    /// `userFills`: the latest, newest first, which replace what is held.
    Fills { user: Address, fills: &'a [crate::account::fills::Fill] },
    /// `userFillsByTime`: a window, oldest first, added to what is held.
    MoreFills { user: Address, fills: &'a [crate::account::fills::Fill] },
    /// `userFunding`: a window, oldest first, added to what is held.
    Fundings { user: Address, fundings: &'a [UserFunding] },
    /// `userNonFundingLedgerUpdates`: a window, oldest first, added to what is held, in the
    /// socket's row type.
    Ledger { user: Address, rows: &'a [LedgerUpdate] },
}

/// What a frame or a read did to the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folded {
    /// Something a view reads changed.
    Changed,
    /// Told again, and nothing changed but when it was heard.
    Same,
    /// Not kept: nothing wants it, it is older than what is held, or it is not state.
    Dropped,
}

/// What [`Store::sync`] gives the socket's owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Synced {
    /// The frames to send, in order.
    pub frames: Vec<String>,
    /// What the session would not take; asked for again on the next sync.
    pub refused: Vec<(Subscription, Refused)>,
}

/// A store and a session on two networks: one store is one network and never feeds a session on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this store is on {store} and the session on {session}; one store is one network")]
pub struct OtherNetwork {
    pub store: Network,
    pub session: Network,
}

/// One network's live state.
#[derive(Debug)]
pub struct Store {
    network: Network,
    interests: Interests,
    markets: Markets,
    accounts: BTreeMap<Address, Account>,
}

impl Store {
    /// An empty store on `network`, within the venue's whole per-IP limits.
    #[must_use]
    pub fn new(network: Network) -> Store {
        Store {
            network,
            interests: Interests::new(Limits::VENUE),
            markets: Markets::default(),
            accounts: BTreeMap::new(),
        }
    }

    /// Within a part of the IP's limits, for a store sharing it with other sessions.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Store {
        self.interests.set_limits(limits);
        self
    }

    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }

    /// Needs `interest` kept live until the handle is dropped or released. Refused past the
    /// venue's limits, or when the venue would not take a subscription it needs.
    pub fn register(&mut self, interest: Interest) -> Result<Handle, Refused> {
        self.settle();
        self.interests.register(interest)
    }

    /// Ends the interest `handle` holds, as dropping it does, and lets go at once of what
    /// nothing needs any more.
    pub fn release(&mut self, handle: Handle) {
        drop(handle);
        self.settle();
    }

    /// Every subscription wanted now, in the order first wanted.
    #[must_use]
    pub fn wanted(&self) -> &[Subscription] {
        self.interests.wanted()
    }

    /// The accounts followed, in the order first followed; at most the limits' users.
    #[must_use]
    pub fn followed(&self) -> &[Address] {
        self.interests.users()
    }

    /// What to tell the socket since it was last told: unsubscribe what nothing needs, then
    /// subscribe what is newly needed.
    pub fn changes(&mut self) -> Changes {
        self.settle();
        self.interests.changes()
    }

    /// [`Store::changes`] into `session`: the frames to send now. A session on another network
    /// is refused before anything changes.
    pub fn sync(&mut self, session: &mut Session) -> Result<Synced, OtherNetwork> {
        if session.network() != self.network {
            return Err(OtherNetwork { store: self.network, session: session.network() });
        }
        let changes = self.changes();
        let mut synced = Synced::default();
        for subscription in &changes.unsubscribe {
            synced.frames.extend(session.unsubscribe(subscription));
        }
        for subscription in changes.subscribe {
            match session.subscribe(subscription.clone()) {
                Ok(frame) => synced.frames.extend(frame),
                Err(refused) => {
                    self.interests.refused(&subscription);
                    synced.refused.push((subscription, refused));
                }
            }
        }
        Ok(synced)
    }

    /// A frame from the socket, received at `now` milliseconds. `userEvents` and `orderUpdates`
    /// frames name no account, so with several followed they are not placed: fills, fundings and
    /// resting orders come from their own channels.
    pub fn apply(&mut self, now: u64, frame: &Frame) -> Folded {
        self.settle();
        match frame {
            Frame::AllMids(mids) => self.fold_mids(now, &mids.dex, &mids.mids),
            Frame::Book(book) => self.fold_book(now, book.clone()),
            Frame::Trades(prints) => self.fold_prints(now, prints.iter().rev().cloned()),
            Frame::Candle(candle) => self.fold_candles(now, [candle.clone()]),
            Frame::AssetCtx { coin, ctx } => self.fold_perp_ctxs(now, [(coin.clone(), ctx.clone())]),
            Frame::SpotAssetCtx { coin, ctx } => self.fold_spot_ctxs(now, [(coin.clone(), ctx.clone())]),
            Frame::AllDexsAssetCtxs(dexes) => dexes
                .iter()
                .map(|(dex, ctxs)| self.fold_dex_ctxs(now, dex, ctxs.iter().cloned()))
                .fold(Folded::Dropped, Folded::or),
            Frame::AssetCtxs { dex, ctxs } => self.fold_dex_ctxs(now, dex, ctxs.iter().cloned()),
            Frame::SpotAssetCtxs(ctxs) => {
                self.fold_spot_ctxs(now, ctxs.iter().map(|ctx| (ctx.coin.clone(), ctx.clone())))
            }
            Frame::ClearinghouseState(one) => {
                let state = one.state.clone();
                self.with_state(one.user, |account| account.set_dex(now, &one.dex, state))
            }
            Frame::AllDexsClearinghouseState(all) => {
                let states = all.states.clone();
                self.with_state(all.user, |account| account.set_dexes(now, states))
            }
            Frame::SpotState(spot) => {
                let wanted = Subscription::SpotState { user: spot.user, ignore_portfolio_margin: false };
                self.with(spot.user, &wanted, |account| {
                    let spot = convert::spot_state(spot, account.spot().map(|held| &held.value));
                    account.set_spot(now, spot)
                })
            }
            Frame::WebData3(web) => match web.user_state.abstraction.as_deref() {
                Some(mode) => self.fold_mode(now, web.user_state.user, Abstraction::parse(mode)),
                None => Folded::Dropped,
            },
            Frame::OpenOrders(open) => {
                let orders = open.orders.clone();
                self.fold_orders(now, open.user, &open.dex, orders)
            }
            Frame::UserFills(fills) => {
                let newest = fills.fills.iter().rev().cloned().collect();
                self.fold_fills(now, fills.user, newest, fills.is_snapshot)
            }
            Frame::UserFundings(fundings) => {
                let wanted = Subscription::UserFundings { user: fundings.user };
                let newest = fundings.fundings.iter().rev().cloned().collect();
                let whole = fundings.is_snapshot;
                self.with(fundings.user, &wanted, |account| account.fold_fundings(now, newest, whole))
            }
            Frame::Ledger(ledger) => {
                let newest = ledger.updates.iter().rev().cloned().collect();
                self.fold_ledger(now, ledger.user, newest, ledger.is_snapshot)
            }
            _ => Folded::Dropped,
        }
    }

    /// A typed `/info` answer, read at `now` milliseconds. A read of something no interest wants
    /// (a book, an account) is not kept; mids, contexts and the metas are kept whatever reads
    /// them.
    pub fn read(&mut self, now: u64, read: Read<'_>) -> Folded {
        self.settle();
        match read {
            Read::Book(book) => self.fold_book(now, (*book).clone()),
            Read::Trades(prints) => self.fold_prints(now, prints.iter().cloned()),
            Read::Candles(candles) => self.fold_history(now, candles.to_vec()),
            Read::Mids { dex, mids } => self.fold_mids(now, dex, mids),
            Read::Perps(read) => {
                let coins: Vec<String> = read.meta.universe.iter().map(|perp| perp.name.clone()).collect();
                let listed = self.list("", coins, read.meta.collateral_token);
                listed.or(self.fold_dex_ctxs(now, "", read.ctxs.iter().cloned()))
            }
            Read::Dex { dex, read } => {
                let coins: Vec<String> = read.meta.universe.iter().map(|perp| perp.name.clone()).collect();
                let listed = self.list(dex, coins, read.meta.collateral_token);
                listed.or(self.fold_dex_ctxs(now, dex, read.ctxs.iter().cloned()))
            }
            Read::Dexes(dexes) => dexes
                .list
                .0
                .iter()
                .zip(&dexes.metas)
                .map(|(dex, meta)| {
                    let name = dex.as_ref().map_or("", |dex| dex.name.as_str());
                    let coins = meta.universe.iter().map(|perp| perp.name.clone()).collect();
                    self.list(name, coins, meta.collateral_token)
                })
                .fold(Folded::Dropped, Folded::or),
            Read::Spot(read) => {
                let listed = self.fold_spot_meta(&read.meta);
                listed.or(self.fold_spot_ctxs(now, read.ctxs.iter().map(|ctx| (ctx.coin.clone(), ctx.clone()))))
            }
            Read::SpotMeta(meta) => self.fold_spot_meta(meta),
            Read::Clearinghouse { user, dex, state } => {
                self.with_state(user, |account| account.set_dex(now, dex, state.clone()))
            }
            Read::SpotState { user, state } => {
                let wanted = Subscription::SpotState { user, ignore_portfolio_margin: false };
                self.with(user, &wanted, |account| account.set_spot(now, state.clone()))
            }
            Read::Mode { user, mode } => self.fold_mode(now, user, mode.clone()),
            Read::Orders { user, dex, orders } => self.fold_orders(now, user, dex, orders.to_vec()),
            Read::Fills { user, fills } => self.fold_fills(now, user, fills.to_vec(), true),
            Read::MoreFills { user, fills } => self.fold_fills(now, user, fills.iter().rev().cloned().collect(), false),
            Read::Fundings { user, fundings } => {
                let wanted = Subscription::UserFundings { user };
                let newest = fundings.iter().rev().map(convert::funding).collect();
                self.with(user, &wanted, |account| account.fold_fundings(now, newest, false))
            }
            Read::Ledger { user, rows } => self.fold_ledger(now, user, rows.iter().rev().cloned().collect(), false),
        }
    }

    /// Ends the interests whose handles were dropped, and lets go of what nothing wants.
    fn settle(&mut self) {
        if !self.interests.drain() {
            return;
        }
        let interests = &self.interests;
        self.markets.books.retain(|coin, _| interests.wants(&book(coin)));
        self.markets.tapes.retain(|coin, _| interests.wants(&Subscription::Trades { coin: coin.clone() }));
        self.markets.candles.retain(|(coin, interval), _| {
            interests.wants(&Subscription::Candle { coin: coin.clone(), interval: *interval })
        });
        let users = interests.users();
        self.accounts.retain(|user, _| users.contains(user));
        for (user, account) in &mut self.accounts {
            let user = *user;
            let orders = |dex: &str| interests.wants(&Subscription::OpenOrders { user, dex: dex.to_owned() });
            account.keep(&Keep {
                state: interests.wants(&Subscription::AllDexsClearinghouseState { user }),
                orders: &orders,
                fills: interests.wants(&Subscription::UserFills { user, aggregate_by_time: false }),
                fundings: interests.wants(&Subscription::UserFundings { user }),
                ledger: interests.wants(&Subscription::UserNonFundingLedgerUpdates { user }),
            });
        }
    }

    fn fold_mids(&mut self, now: u64, dex: &str, mids: &Mids) -> Folded {
        let mids: BTreeMap<String, Dec> = mids.iter().map(|(coin, mid)| (coin.to_owned(), mid)).collect();
        match self.markets.mids.get_mut(dex) {
            Some(held) => Folded::of(held.set(now, mids)),
            None => {
                self.markets.mids.insert(dex.to_owned(), Stamped::new(now, mids));
                Folded::Changed
            }
        }
    }

    /// A book replaces the one held unless it is older by its own time. A grouped book (one
    /// with a spread) is not the full book the store asks for.
    fn fold_book(&mut self, now: u64, sent: Book) -> Folded {
        if sent.spread.is_some() || !self.interests.wants(&book(&sent.coin)) {
            return Folded::Dropped;
        }
        match self.markets.books.get_mut(&sent.coin) {
            Some(held) if held.time > sent.time => Folded::Dropped,
            Some(held) => Folded::of(held.set(now, sent)),
            None => {
                self.markets.books.insert(sent.coin.clone(), Stamped::new(now, sent));
                Folded::Changed
            }
        }
    }

    /// Prints, newest first, onto each wanted coin's tape.
    fn fold_prints(&mut self, now: u64, prints: impl IntoIterator<Item = Trade>) -> Folded {
        let mut by_coin: BTreeMap<String, Vec<Trade>> = BTreeMap::new();
        for print in prints {
            by_coin.entry(print.coin.clone()).or_default().push(print);
        }
        let mut folded = Folded::Dropped;
        for (coin, prints) in by_coin {
            if !self.interests.wants(&Subscription::Trades { coin: coin.clone() }) {
                continue;
            }
            let tape = self.markets.tapes.entry(coin).or_insert_with(|| Stamped::new(now, Vec::new()));
            let changed = fold_prints(&mut tape.value, prints);
            tape.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// Candles from the socket, each onto its wanted market and interval.
    fn fold_candles(&mut self, now: u64, candles: impl IntoIterator<Item = Candle>) -> Folded {
        let mut folded = Folded::Dropped;
        for candle in candles {
            let key = (candle.coin.clone(), candle.interval);
            if !self.interests.wants(&Subscription::Candle { coin: key.0.clone(), interval: key.1 }) {
                continue;
            }
            let bars = self.markets.candles.entry(key).or_insert_with(|| Stamped::new(now, Vec::new()));
            let changed = fold_bar(&mut bars.value, candle);
            bars.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// Candles read, oldest first: the history goes under what the socket already sent, and
    /// the socket's win.
    fn fold_history(&mut self, now: u64, history: Vec<Candle>) -> Folded {
        let mut by_market: BTreeMap<(String, Interval), Vec<Candle>> = BTreeMap::new();
        for candle in history {
            by_market.entry((candle.coin.clone(), candle.interval)).or_default().push(candle);
        }
        let mut folded = Folded::Dropped;
        for (key, mut history) in by_market {
            if !self.interests.wants(&Subscription::Candle { coin: key.0.clone(), interval: key.1 }) {
                continue;
            }
            let bars = self.markets.candles.entry(key).or_insert_with(|| Stamped::new(now, Vec::new()));
            history.sort_by_key(|candle| candle.open_time);
            let live = std::mem::take(&mut bars.value);
            let mut merged = history;
            for bar in live.iter().cloned() {
                let _ = fold_bar(&mut merged, bar);
            }
            if merged.len() > BARS {
                let extra = merged.len() - BARS;
                merged.drain(..extra);
            }
            let changed = merged != live;
            bars.value = merged;
            bars.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    fn fold_perp_ctxs(&mut self, now: u64, ctxs: impl IntoIterator<Item = (String, PerpCtx)>) -> Folded {
        let mut folded = Folded::Dropped;
        for (coin, ctx) in ctxs {
            let changed = match self.markets.perps.get_mut(&coin) {
                Some(held) => held.set(now, ctx),
                None => {
                    self.markets.perps.insert(coin, Stamped::new(now, ctx));
                    true
                }
            };
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    fn fold_spot_ctxs(&mut self, now: u64, ctxs: impl IntoIterator<Item = (String, SpotCtx)>) -> Folded {
        let mut folded = Folded::Dropped;
        for (coin, ctx) in ctxs {
            let changed = match self.markets.spots.get_mut(&coin) {
                Some(held) => held.set(now, ctx),
                None => {
                    self.markets.spots.insert(coin, Stamped::new(now, ctx));
                    true
                }
            };
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// One dex's contexts in its universe's order. With no universe read for the dex there is
    /// nothing to place them on; past its end (a perp listed since) they wait for the next read.
    fn fold_dex_ctxs(&mut self, now: u64, dex: &str, ctxs: impl IntoIterator<Item = PerpCtx>) -> Folded {
        let Some(coins) = self.markets.universes.get(dex) else { return Folded::Dropped };
        let placed: Vec<(String, PerpCtx)> = coins.iter().cloned().zip(ctxs).collect();
        self.fold_perp_ctxs(now, placed)
    }

    /// A dex's universe and collateral, from its meta.
    fn list(&mut self, dex: &str, coins: Vec<String>, collateral: Option<u32>) -> Folded {
        let token = collateral.unwrap_or(USDC_INDEX);
        let changed = self.markets.universes.get(dex) != Some(&coins) || self.markets.collateral.of(dex) != token;
        self.markets.universes.insert(dex.to_owned(), coins);
        self.markets.collateral = std::mem::take(&mut self.markets.collateral).with(dex, token);
        Folded::of(changed)
    }

    fn fold_spot_meta(&mut self, meta: &SpotMeta) -> Folded {
        let changed = self.markets.spot_meta.as_ref() != Some(meta);
        self.markets.spot_meta = Some(meta.clone());
        Folded::of(changed)
    }

    fn fold_mode(&mut self, now: u64, user: Address, mode: Abstraction) -> Folded {
        self.with(user, &Subscription::WebData3 { user }, |account| account.set_mode(now, mode))
    }

    fn fold_orders(&mut self, now: u64, user: Address, dex: &str, orders: Vec<FrontendOrder>) -> Folded {
        let wanted = Subscription::OpenOrders { user, dex: dex.to_owned() };
        self.with(user, &wanted, |account| account.set_orders(now, dex, orders))
    }

    fn fold_fills(&mut self, now: u64, user: Address, fills: Vec<crate::live::Fill>, whole: bool) -> Folded {
        let wanted = Subscription::UserFills { user, aggregate_by_time: false };
        self.with(user, &wanted, |account| account.fold_fills(now, fills, whole))
    }

    fn fold_ledger(&mut self, now: u64, user: Address, rows: Vec<LedgerUpdate>, whole: bool) -> Folded {
        let wanted = Subscription::UserNonFundingLedgerUpdates { user };
        self.with(user, &wanted, |account| account.fold_ledger(now, rows, whole))
    }

    /// Changes `user`'s clearinghouse, while its state is wanted.
    fn with_state(&mut self, user: Address, change: impl FnOnce(&mut Account) -> bool) -> Folded {
        self.with(user, &Subscription::AllDexsClearinghouseState { user }, change)
    }

    /// Changes `user`'s account while `wanted` is: what no interest asked for is not kept.
    fn with(&mut self, user: Address, wanted: &Subscription, change: impl FnOnce(&mut Account) -> bool) -> Folded {
        if !self.interests.wants(wanted) {
            return Folded::Dropped;
        }
        Folded::of(change(self.accounts.entry(user).or_default()))
    }

    // Views: pure reads of the state.

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

impl Folded {
    const fn of(changed: bool) -> Folded {
        match changed {
            true => Folded::Changed,
            false => Folded::Same,
        }
    }

    /// The more of the two: a change over a repeat over nothing kept.
    const fn or(self, other: Folded) -> Folded {
        match (self, other) {
            (Folded::Changed, _) | (_, Folded::Changed) => Folded::Changed,
            (Folded::Same, _) | (_, Folded::Same) => Folded::Same,
            _ => Folded::Dropped,
        }
    }
}

/// The full book's subscription, the one [`Interest::Book`] asks for.
fn book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}
