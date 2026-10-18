//! A frame or an `/info` read into the store: [`Store::apply`] and [`Store::read`] hand each to
//! the fold for its kind of data.

use crate::account::Abstraction;
use crate::live::store::convert;
use crate::live::store::{Folded, Read, Store};
use crate::live::{Frame, Subscription};

impl Store {
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
}
