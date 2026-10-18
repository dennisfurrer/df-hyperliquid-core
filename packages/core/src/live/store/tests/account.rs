//! One account's histories in the store: fills, fundings and ledger rows held newest first and
//! once each, a snapshot or a whole read replacing them and anything else adding to them, and
//! each piece kept while an interest wants it.

use crate::account::history::LedgerUpdate;
use crate::client::Network;
use crate::live::Frame;
use crate::live::store::fixtures::{FIRST, SECOND, USER_FILLS, USER_FUNDING, address, changed, dec, frame, read};
use crate::live::store::{Folded, Handle, Interest, Read, Store};
use crate::markets::perps::UserFunding;
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

/// The recorded snapshot (30 fills, oldest first) is kept newest first, the fills of one
/// transaction sharing a hash all kept; the live fill after it is added once. A fill is the same
/// fill by its `tid` and `oid`: the two sides of one trade share a `tid` and can both be the
/// account's. A snapshot is the whole history again.
#[test]
fn fills_from_the_socket_are_held_newest_first_once_each() {
    let mut store = store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    assert_eq!(store.apply(1_000, &frame("userFills.liquidation")), Folded::Changed);
    let held = fills(&store, second());
    assert_eq!((held.len(), held[0]), (30, (1791480439114, 359101774658102)));
    assert!(held.windows(2).all(|pair| pair[0].0 >= pair[1].0), "newest first");
    let hash = "0x179eeccffddc7852191804461fff2d02121c00b598df9724bb679822bcd0523c";
    let account = store.account(second()).unwrap();
    assert_eq!(account.fills().unwrap().iter().filter(|fill| fill.hash == hash).count(), 2);
    assert_eq!(store.apply(2_000, &frame("userFills")), Folded::Changed);
    assert_eq!(fills(&store, second())[0], (1791480468944, 167376889247413));
    assert_eq!(store.apply(3_000, &frame("userFills")), Folded::Same, "the same fill again");
    let kept = store.account(second()).unwrap().fills().unwrap();
    assert_eq!((kept.len(), kept.heard, kept.changed), (31, 3_000, 2_000));
    assert_eq!(kept.iter().filter(|fill| fill.liquidation.is_some()).count(), 3, "liquidations kept on their fills");

    let mut store = self::store();
    let _fills = wanting(&mut store, [Interest::Fills { user: second() }]);
    let _ = store.apply(1, &fills_frame(true, &[(1, 1), (2, 2)]));
    let _ = store.apply(2, &fills_frame(false, &[(2, 2), (3, 3)]));
    assert_eq!(fills(&store, second()).len(), 3, "a live frame is news, an overlap is not counted twice");
    let _ = store.apply(3, &fills_frame(false, &[(5, 1), (5, 2)]));
    assert_eq!(fills(&store, second()).len(), 5, "both sides of one trade are two fills");
    let _ = store.apply(4, &fills_frame(true, &[(9, 9)]));
    assert_eq!(fills(&store, second()), [(1791480468944, 9)], "a snapshot is the whole history again");
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
/// again it is the same; a `userFunding` read adds its window, each payment once. The recorded
/// ledger snapshot (23 rows) likewise, and a live row is added once.
#[test]
fn fundings_and_ledger_rows_are_held_newest_first_once_each() {
    let mut store = store();
    let hlp = address("0x010461c14e146ac35fe42271bdc1134ee31c703a");
    let _held = wanting(
        &mut store,
        [Interest::Fundings { user: first() }, Interest::Fundings { user: hlp }, Interest::Ledger { user: first() }],
    );
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

/// An account's pieces go with the interest that wanted them; the rest stay. It was last heard
/// when any piece was; its standing, when its state was.
#[test]
fn an_account_s_pieces_go_with_their_interests() {
    let mut store = store();
    let mut held = wanting(&mut store, [Interest::State { user: second() }, Interest::Fills { user: second() }]);
    let _ = store.apply(1_000, &frame("clearinghouseState"));
    let _ = store.apply(5_000, &frame("userFills"));
    let account = store.account(second()).unwrap();
    assert_eq!((account.heard(), account.standing().heard), (Some(5_000), Some(1_000)));
    store.release(held.remove(1));
    let account = store.account(second()).unwrap();
    assert!(account.fills().is_none());
    assert_eq!(account.positions().count(), 1);
    assert_eq!(store.apply(6_000, &frame("userFills")), Folded::Dropped);
}
