use crate::account::{Abstraction, SpotState};
use crate::account::{ClearinghouseState, Leverage};
use crate::account::{FrontendOrder, OrderKind};
use crate::client::Network;
use crate::live::store::fixtures::{
    FIRST, FRONTEND_OPEN_ORDERS, HOLDER, SECOND, SPOT_PORTFOLIO_MARGIN, address, answered, changed, dec, frame,
    portfolio_frame, read,
};
use crate::live::store::{Folded, Handle, Interest, Read, Store};
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
    let holder = address(HOLDER);
    let _state = wanting(&mut store, [Interest::State { user: holder }]);
    let _ = store.apply(1_000, &portfolio_frame("allDexsClearinghouseState"));
    let btc = store.positions(holder)[0].clone();
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    let held = &store.account(holder).unwrap().standing().dex("").unwrap().asset_positions[0].position;
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
