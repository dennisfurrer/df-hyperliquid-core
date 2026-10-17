use crate::account::{Abstraction, SpotState};
use crate::account::{ClearinghouseState, Leverage};
use crate::account::{FrontendOrder, OrderKind};
use crate::client::Network;
use crate::live::store::fixtures::{
    FIRST, FRONTEND_OPEN_ORDERS, SECOND, SPOT_PORTFOLIO_MARGIN, TUI, USER_FILLS, USER_FUNDING, address, answered,
    changed, dec, frame, read, tui_frame,
};
use crate::live::store::{Folded, Handle, Interest, Read, Store};
use crate::live::{Frame, LedgerUpdate};
use crate::markets::perps::UserFunding;
use crate::trade::Cloid;
use crate::utils::Address;

fn store() -> Store {
    Store::new(Network::Mainnet)
}

fn wanting(store: &mut Store, interests: impl IntoIterator<Item = Interest>) -> Vec<Handle> {
    interests.into_iter().map(|interest| store.register(interest).unwrap()).collect()
}

fn second() -> Address {
    address(SECOND)
}

fn first() -> Address {
    address(FIRST)
}

/// The fills held for `user`: (time, tid), newest first.
fn fills(store: &Store, user: Address) -> Vec<(u64, u64)> {
    store
        .account(user)
        .and_then(|account| account.fills())
        .map_or_else(Vec::new, |fills| fills.iter().map(|fill| (fill.time, fill.tid)).collect())
}

/// A `userFills` frame for SECOND with the recorded live fill once for each `(tid, oid)`.
fn fills_frame(snapshot: bool, ids: &[(u64, u64)]) -> Frame {
    changed("userFills", |json| {
        let one = json["data"]["fills"][0].clone();
        let fills: Vec<serde_json::Value> = ids
            .iter()
            .map(|(tid, oid)| {
                let mut fill = one.clone();
                fill["tid"] = (*tid).into();
                fill["oid"] = (*oid).into();
                fill
            })
            .collect();
        json["data"]["fills"] = fills.into();
        json["data"]["isSnapshot"] = snapshot.into();
    })
}

/// The recorded snapshot (30 fills, oldest first) is kept newest first; the live fill after it
/// is added; the same live fill from a second frame is not added again.
#[test]
fn a_fills_snapshot_then_live_fills_each_held_once() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    assert_eq!(store.apply(1_000, &frame("userFills.liquidation")), Folded::Changed);
    let held = fills(&store, second());
    assert_eq!((held.len(), held[0]), (30, (1791480439114, 359101774658102)));
    assert!(held.windows(2).all(|pair| pair[0].0 >= pair[1].0), "newest first");
    assert_eq!(store.apply(2_000, &frame("userFills")), Folded::Changed);
    assert_eq!(fills(&store, second())[0], (1791480468944, 167376889247413));
    assert_eq!(store.apply(3_000, &frame("userFills")), Folded::Same, "the same fill again");
    let account = store.account(second()).unwrap();
    let kept = account.fills().unwrap();
    assert_eq!((kept.len(), kept.heard, kept.changed), (31, 3_000, 2_000));
    assert_eq!(kept.iter().filter(|fill| fill.liquidation.is_some()).count(), 3, "liquidations kept on their fills");
}

/// One order crossing several levels makes several fills in one transaction: they share a hash
/// and are all kept (the recorded snapshot has two such pairs).
#[test]
fn fills_of_one_transaction_share_a_hash_and_are_all_kept() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1_000, &frame("userFills.liquidation"));
    let account = store.account(second()).unwrap();
    let hash = "0x179eeccffddc7852191804461fff2d02121c00b598df9724bb679822bcd0523c";
    assert_eq!(account.fills().unwrap().iter().filter(|fill| fill.hash == hash).count(), 2);
}

#[test]
fn a_fills_snapshot_replaces_the_history_and_a_live_frame_extends_it() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1, &fills_frame(true, &[(1, 1), (2, 2)]));
    assert_eq!(fills(&store, second()).len(), 2);
    let _ = store.apply(2, &fills_frame(false, &[(3, 3)]));
    assert_eq!(fills(&store, second()).len(), 3, "a live frame is news");
    let _ = store.apply(3, &fills_frame(true, &[(9, 9)]));
    assert_eq!(fills(&store, second()), [(1791480468944, 9)], "and a snapshot is the whole history again");
}

#[test]
fn fills_are_not_counted_twice_when_frames_overlap() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1, &fills_frame(false, &[(1, 1), (2, 2)]));
    let _ = store.apply(2, &fills_frame(false, &[(2, 2), (3, 3)]));
    assert_eq!(fills(&store, second()).len(), 3);
}

/// The two sides of one trade share its `tid`; an account on both sides of it (its own orders
/// crossing) has two fills, told apart by their orders.
#[test]
fn both_sides_of_one_trade_are_two_fills() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1, &fills_frame(false, &[(5, 1), (5, 2)]));
    assert_eq!(fills(&store, second()).len(), 2);
}

/// `userFills` read whole replaces what is held; `userFillsByTime` adds a window to it.
#[test]
fn a_fills_read_is_a_whole_and_a_window_adds_to_it() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1_000, &frame("userFills.liquidation"));
    let recorded: Vec<crate::account::history::Fill> = read(USER_FILLS);
    assert_eq!(store.read(2_000, Read::Fills { user: second(), fills: &recorded }), Folded::Changed);
    assert_eq!(fills(&store, second()).len(), 5, "the read is the whole of the latest");
    assert_eq!(store.read(3_000, Read::MoreFills { user: second(), fills: &recorded }), Folded::Same);
    let _ = store.apply(4_000, &frame("userFills"));
    assert_eq!(store.read(5_000, Read::MoreFills { user: second(), fills: &recorded[..2] }), Folded::Same);
    let held = store.account(second()).unwrap().fills().unwrap();
    assert_eq!(held.len(), 6);
    assert_eq!((held[0].tid, held[1].coin.as_str(), held[1].cloid.as_deref()), (167376889247413, "GRAM", None));
}

/// The recorded `userFundings` snapshot (100 payments, oldest first) is kept newest first; told
/// again it is the same; a `userFunding` read adds its window, each payment once.
#[test]
fn fundings_snapshot_then_reads_each_held_once() {
    let mut store = store();
    let hlp = address("0x010461c14e146ac35fe42271bdc1134ee31c703a");
    let _fundings = wanting(&mut store, [Interest::Fundings { user: first() }, Interest::Fundings { user: hlp }]);
    assert_eq!(store.apply(1_000, &frame("userFundings")), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("userFundings")), Folded::Same);
    let held = store.account(first()).unwrap().fundings().unwrap();
    assert_eq!((held.len(), held[0].time, held[0].coin.as_str()), (100, 1791478800051, "xyz:BRENTOIL"));
    let window: Vec<UserFunding> = read(USER_FUNDING);
    assert_eq!(store.read(3_000, Read::Fundings { user: hlp, fundings: &window }), Folded::Changed);
    assert_eq!(store.read(4_000, Read::Fundings { user: hlp, fundings: &window }), Folded::Same);
    let held = store.account(hlp).unwrap().fundings().unwrap();
    let rows: Vec<(&str, _)> = held.iter().map(|one| (one.coin.as_str(), one.usdc)).collect();
    assert_eq!(
        rows,
        [("DYDX", dec("0.44349")), ("ETH", dec("0.107324")), ("BTC", dec("-0.495632"))],
        "read oldest first"
    );
}

/// The recorded ledger snapshot (23 rows) is kept newest first; a live row is added once.
#[test]
fn ledger_snapshot_then_live_rows_each_held_once() {
    let mut store = store();
    let _ledger = wanting(&mut store, [Interest::Ledger { user: first() }]);
    assert_eq!(store.apply(1_000, &frame("userNonFundingLedgerUpdates")), Folded::Changed);
    let newest = store.account(first()).unwrap().ledger().unwrap()[0].clone();
    assert_eq!((newest.time, newest.kind()), (1790318031225, "send"));
    // The newest row, a day later, sent live.
    let live = changed("userNonFundingLedgerUpdates", |json| {
        let mut row = json["data"]["nonFundingLedgerUpdates"][22].clone();
        row["time"] = 1790404431225u64.into();
        json["data"]["nonFundingLedgerUpdates"] = vec![row].into();
        json["data"]["isSnapshot"] = false.into();
    });
    assert_eq!(store.apply(2_000, &live), Folded::Changed);
    assert_eq!(store.apply(3_000, &live), Folded::Same);
    let rows: Vec<LedgerUpdate> = store.account(first()).unwrap().ledger().unwrap().value.clone();
    assert_eq!((rows.len(), rows[0].time), (24, 1790404431225));
    assert_eq!(store.read(4_000, Read::Ledger { user: first(), rows: &rows }), Folded::Same);
}

/// One dex's clearinghouse from the socket, and later the same account's read over a post: its
/// position as the venue stated it each time.
#[test]
fn a_dex_s_clearinghouse_gives_its_positions() {
    let mut store = store();
    let _state = wanting(&mut store, [Interest::State { user: second() }]);
    assert_eq!(store.apply(1_000, &frame("clearinghouseState")), Folded::Changed);
    let positions = store.positions(second());
    let btc = positions[0];
    assert_eq!((btc.coin.as_str(), btc.szi, btc.entry_px), ("BTC", dec("-12.07087"), Some(dec("81063.7"))));
    assert_eq!(
        (btc.return_on_equity, btc.leverage.clone()),
        (Some(dec("0.0722953617")), Leverage::Cross { value: 11 })
    );
    assert_eq!(btc.cum_funding.as_ref().map(|funding| funding.since_open), Some(dec("-973.335874")));
    let read_later: ClearinghouseState = read(&answered("post.info.clearinghouseState"));
    let told = Read::Clearinghouse { user: second(), dex: "", state: &read_later };
    assert_eq!(store.read(2_000, told), Folded::Changed);
    assert_eq!(store.positions(second())[0].szi, dec("-12.45034"));
    let dex = store.account(second()).unwrap().dex("").unwrap();
    assert_eq!((dex.withdrawable, dex.heard), (dec("0.0"), 2_000));
}

/// Every dex in one frame: eleven on mainnet, one position among them.
#[test]
fn every_dex_s_clearinghouse_in_one_frame() {
    let mut store = store();
    let _state = wanting(&mut store, [Interest::State { user: second() }]);
    assert_eq!(store.apply(1_000, &frame("allDexsClearinghouseState")), Folded::Changed);
    let account = store.account(second()).unwrap();
    let dexes: Vec<&str> = account.dexes().map(|(dex, _)| dex).collect();
    assert_eq!(dexes, ["", "abcd", "cash", "flx", "hyna", "io", "km", "mkts", "para", "vntl", "xyz"]);
    assert_eq!(store.positions(second()).len(), 1);
    assert_eq!(account.standing().dexes.len(), 11, "the account reads the same dexes");
    assert_eq!(account.standing().dex("").unwrap().margin_summary.account_value, dec("99248.48428"));
}

/// The 2026-09-26 recording: an isolated BTC long carries the margin put up.
#[test]
fn an_isolated_position_keeps_the_margin_put_up() {
    let mut store = store();
    let tui = address(TUI);
    let _state = wanting(&mut store, [Interest::State { user: tui }]);
    let _ = store.apply(1_000, &tui_frame("allDexsClearinghouseState"));
    let btc = store.positions(tui)[0].clone();
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    let held = &store.account(tui).unwrap().standing().dex("").unwrap().asset_positions[0].position;
    assert_eq!(held.leverage, btc.leverage, "the account view holds the one type");
}

/// Resting orders as `openOrders` sends them, in the `frontendOpenOrders` row type.
#[test]
fn resting_orders_come_typed_and_an_empty_list_empties_them() {
    let mut store = store();
    let _orders = wanting(&mut store, [Interest::Orders { user: second(), dex: String::new() }]);
    assert_eq!(store.apply(1_000, &frame("openOrders")), Folded::Changed);
    let orders: Vec<FrontendOrder> = store.orders(second()).into_iter().cloned().collect();
    assert_eq!(orders.len(), 1);
    let order = &orders[0];
    assert_eq!((order.coin.as_str(), order.oid, order.limit_px), ("BTC", 569017061987, dec("80459.0")));
    assert_eq!((order.order_type.clone(), order.tif.as_deref()), (OrderKind::Limit, Some("Alo")));
    assert_eq!(order.cloid, Some("0x913a0c81d251e55d3792962e03a96f00".parse::<Cloid>().unwrap()));
    assert_eq!(store.apply(2_000, &frame("openOrders.empty")), Folded::Changed);
    assert!(store.orders(second()).is_empty());
    assert!(store.account(second()).unwrap().orders("").is_some(), "told none, not never told");
}

/// `frontendOpenOrders` read: a position's stop and a spot limit.
#[test]
fn a_resting_orders_read_keeps_triggers() {
    let mut store = store();
    let _orders = wanting(&mut store, [Interest::Orders { user: first(), dex: String::new() }]);
    let recorded: Vec<FrontendOrder> = read(FRONTEND_OPEN_ORDERS);
    assert_eq!(store.read(1_000, Read::Orders { user: first(), dex: "", orders: &recorded }), Folded::Changed);
    let orders = store.orders(first());
    assert_eq!((orders[0].order_type.clone(), orders[0].trigger()), (OrderKind::StopMarket, Some(dec("6.04"))));
    assert_eq!((orders[1].coin.as_str(), orders[1].tif.as_deref()), ("@142", Some("Gtc")));
    assert_eq!(store.read(2_000, Read::Orders { user: first(), dex: "xyz", orders: &recorded }), Folded::Dropped);
}

/// The socket's spot balances say nothing of portfolio margin; what a read said of it stays.
#[test]
fn spot_balances_from_the_socket_keep_what_a_read_said_of_portfolio_margin() {
    let mut store = store();
    let _state = wanting(&mut store, [Interest::State { user: first() }]);
    let margined: SpotState = read(SPOT_PORTFOLIO_MARGIN);
    assert_eq!(store.read(1_000, Read::SpotState { user: first(), state: &margined }), Folded::Changed);
    assert_eq!(store.apply(2_000, &frame("spotState")), Folded::Changed);
    let spot = store.account(first()).unwrap().spot().unwrap();
    assert_eq!(
        (spot.balances.len(), spot.balances[0].total, spot.balances[0].hold),
        (4, dec("19042.782306"), dec("9001.9958"))
    );
    assert_eq!(spot.portfolio_margin_enabled, Some(true));
    assert_eq!(spot.portfolio_margin_ratio, Some(dec("0.1460346663")));
    assert_eq!(spot.available_after_maintenance(0), Some(dec("16840.184972")), "the socket's own");
}

#[test]
fn the_mode_comes_with_web_data_and_a_frame_without_one_keeps_it() {
    let mut store = store();
    let _state = wanting(&mut store, [Interest::State { user: second() }]);
    assert_eq!(store.apply(1_000, &frame("webData3")), Folded::Changed);
    assert_eq!(store.account(second()).unwrap().mode().map(|mode| mode.value.clone()), Some(Abstraction::Disabled));
    let silent = changed("webData3", |json| {
        if let Some(state) = json["data"]["userState"].as_object_mut() {
            state.remove("abstraction");
        }
    });
    assert_eq!(store.apply(2_000, &silent), Folded::Dropped);
    let unified = Read::Mode { user: second(), mode: &Abstraction::UnifiedAccount };
    assert_eq!(store.read(3_000, unified), Folded::Changed);
    assert_eq!(store.account(second()).unwrap().standing().mode, Some(Abstraction::UnifiedAccount));
}

#[test]
fn an_account_not_followed_is_not_kept_and_one_let_go_is_gone() {
    let mut store = store();
    assert_eq!(store.apply(1_000, &frame("clearinghouseState")), Folded::Dropped);
    assert!(store.account(second()).is_none());
    let mut state = wanting(&mut store, [Interest::State { user: second() }]);
    let _ = store.apply(1_000, &frame("clearinghouseState"));
    assert!(store.account(second()).is_some());
    store.release(state.remove(0));
    assert!(store.account(second()).is_none());
    assert!(store.followed().is_empty());
}

/// An account's pieces go with the interest that wanted them; the rest stay.
#[test]
fn an_account_s_pieces_go_with_their_interests() {
    let mut store = store();
    let mut held = wanting(&mut store, [Interest::State { user: second() }, Interest::Fills { user: second() }]);
    let _ = store.apply(1_000, &frame("clearinghouseState"));
    let _ = store.apply(1_000, &frame("userFills"));
    store.release(held.remove(1));
    let account = store.account(second()).unwrap();
    assert!(account.fills().is_none());
    assert_eq!(account.positions().count(), 1);
    assert_eq!(store.apply(2_000, &frame("userFills")), Folded::Dropped);
}

#[test]
fn an_account_was_last_heard_when_any_piece_was() {
    let mut store = store();
    let _held = wanting(&mut store, [Interest::State { user: second() }, Interest::Fills { user: second() }]);
    let _ = store.apply(1_000, &frame("clearinghouseState"));
    let _ = store.apply(5_000, &frame("userFills"));
    let account = store.account(second()).unwrap();
    assert_eq!((account.heard(), account.standing().heard), (Some(5_000), Some(1_000)));
}
