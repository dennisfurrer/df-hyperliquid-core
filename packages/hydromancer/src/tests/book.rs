//! The book against Hydromancer's own rules. The first tests are the Python SDK's
//! `tests/test_orderbook.py` (hydromancer-sdk 4371df8), case for case, in exact decimals where
//! the SDK scales by 1e8. The queue tests follow the `insertBefore` rule as the `l4BookUpdates`
//! page states it: in front of that order when it rests at the level, else at the back.

use super::*;

const ONE: &str = "0x1111111111111111111111111111111111111111";
const TWO: &str = "0x2222222222222222222222222222222222222222";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn user(text: &str) -> Address {
    text.parse().unwrap()
}

fn order(oid: u64, who: &str, side: Side, px: i64, sz: i64) -> Order {
    Order { oid, user: user(who), side, px: Dec::from_int(px), sz: Dec::from_int(sz) }
}

fn counts(book: &Book) -> (usize, usize) {
    (book.orders(Side::Bid).count(), book.orders(Side::Ask).count())
}

fn queue(book: &Book, side: Side, px: &str) -> Vec<u64> {
    book.queue(side, dec(px)).map(|order| order.oid).collect()
}

#[test]
fn basic_operations() {
    let mut book = Book::new();
    for oid in 0..3 {
        book.insert(order(oid, ONE, Side::Bid, 5, 100), None);
    }
    for oid in 3..7 {
        book.insert(order(oid, ONE, Side::Bid, 4, 200), None);
    }
    book.insert(order(7, TWO, Side::Ask, 6, 500), None);
    book.insert(order(8, TWO, Side::Ask, 6, 500), None);
    assert_eq!((book.best_bid(), book.best_ask()), (Some(Dec::from_int(5)), Some(Dec::from_int(6))));
    assert_eq!(book.spread(), Some(Dec::ONE));
    assert_eq!(counts(&book), (7, 2));
    assert_eq!(book.len(), 9);
}

#[test]
fn matching_ask_crosses_bids() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Bid, 100, 50), None);
    book.insert(order(2, ONE, Side::Bid, 99, 100), None);
    let crossed = book.insert(order(3, TWO, Side::Ask, 99, 80), None).unwrap();
    assert_eq!((book.best_bid(), book.best_ask()), (Some(Dec::from_int(99)), None));
    assert_eq!(counts(&book), (1, 0));
    assert_eq!(book.get(2).map(|order| order.sz), Some(Dec::from_int(70)));
    assert_eq!(crossed.filled, Dec::from_int(80));
    assert_eq!(crossed.taken.iter().map(|order| order.oid).collect::<Vec<_>>(), [1], "taken whole");
}

#[test]
fn matching_bid_crosses_asks() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Ask, 100, 50), None);
    book.insert(order(2, ONE, Side::Ask, 101, 100), None);
    book.insert(order(3, TWO, Side::Bid, 101, 80), None);
    assert_eq!((book.best_bid(), book.best_ask()), (None, Some(Dec::from_int(101))));
    assert_eq!(counts(&book), (0, 1));
}

#[test]
fn never_crossed() {
    let mut book = Book::new();
    for at in 0..100 {
        book.insert(order(at * 2, ONE, Side::Bid, 100 + i64::try_from(at % 10).unwrap(), 10), None);
        book.insert(order(at * 2 + 1, ONE, Side::Ask, 95 + i64::try_from(at % 10).unwrap(), 10), None);
        if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
            assert!(bid < ask, "crossed at {at}: {bid} >= {ask}");
        }
    }
}

#[test]
fn remove_order() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Bid, 100, 50), None);
    book.insert(order(2, ONE, Side::Bid, 100, 50), None);
    assert_eq!(book.remove(1).map(|order| order.oid), Some(1));
    assert_eq!(counts(&book), (1, 0));
    assert!(book.remove(2).is_some());
    assert_eq!(counts(&book), (0, 0));
    assert!(book.remove(999).is_none());
    assert!(book.levels(Side::Bid).is_empty(), "an emptied level goes");
}

#[test]
fn modify_size() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Bid, 100, 50), None);
    assert!(book.resize(1, Dec::from_int(75)));
    assert_eq!(book.levels(Side::Bid)[0].sz, Dec::from_int(75));
    assert!(!book.resize(999, Dec::from_int(100)));
}

#[test]
fn str_methods() {
    let mut book = Book::new();
    book.insert(Order { oid: 1, user: user(ONE), side: Side::Bid, px: dec("100.5"), sz: dec("1.25") }, None);
    book.insert(Order { oid: 2, user: user(ONE), side: Side::Ask, px: dec("101.75"), sz: dec("2.5") }, None);
    assert_eq!(book.best_bid().map(|px| px.to_string()).as_deref(), Some("100.5"));
    assert_eq!(book.best_ask().map(|px| px.to_string()).as_deref(), Some("101.75"));
    assert_eq!(book.levels(Side::Bid)[0].sz.to_string(), "1.25");
    assert_eq!(book.levels(Side::Ask)[0].sz.to_string(), "2.5");
    assert_eq!(book.spread().map(|spread| spread.to_string()).as_deref(), Some("1.25"));
}

#[test]
fn an_alo_order_joins_in_front_of_the_order_it_names() {
    let mut book = Book::new();
    book.insert(order(10, ONE, Side::Bid, 5, 1), None);
    book.insert(order(11, ONE, Side::Bid, 5, 1), None);
    book.insert(order(12, TWO, Side::Bid, 5, 1), Some(11));
    assert_eq!(queue(&book, Side::Bid, "5"), [10, 12, 11]);
    book.insert(order(13, TWO, Side::Bid, 5, 1), Some(10));
    assert_eq!(queue(&book, Side::Bid, "5"), [13, 10, 12, 11], "in front of the head is the new head");
}

#[test]
fn an_alo_order_naming_an_order_elsewhere_joins_at_the_back() {
    let mut book = Book::new();
    book.insert(order(10, ONE, Side::Bid, 5, 1), None);
    book.insert(order(20, ONE, Side::Bid, 4, 1), None);
    book.insert(order(30, ONE, Side::Ask, 9, 1), None);
    book.insert(order(11, TWO, Side::Bid, 5, 1), Some(20));
    book.insert(order(12, TWO, Side::Bid, 5, 1), Some(999));
    book.insert(order(13, TWO, Side::Bid, 5, 1), Some(30));
    assert_eq!(queue(&book, Side::Bid, "5"), [10, 11, 12, 13]);
}

#[test]
fn a_redelivered_order_is_not_a_second_arrival() {
    let mut book = Book::new();
    assert!(book.insert(order(1, ONE, Side::Bid, 5, 1), None).is_some());
    assert!(book.insert(order(1, TWO, Side::Bid, 6, 9), None).is_none());
    assert_eq!(book.get(1).map(|order| (order.px, order.sz)), Some((Dec::from_int(5), Dec::ONE)));
}

#[test]
fn a_size_cut_to_nothing_takes_the_order_off_the_book() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Ask, 5, 1), None);
    assert!(book.resize(1, Dec::ZERO));
    assert!(book.get(1).is_none() && book.is_empty());
}

#[test]
fn an_owner_s_orders_are_found_on_either_side() {
    let mut book = Book::new();
    book.insert(order(1, ONE, Side::Bid, 5, 1), None);
    book.insert(order(2, TWO, Side::Bid, 5, 1), None);
    book.insert(order(3, ONE, Side::Ask, 7, 1), None);
    let mine = |book: &Book| book.orders_of(user(ONE)).map(|order| order.oid).collect::<Vec<_>>();
    assert_eq!(mine(&book), [1, 3]);
    book.insert(order(4, TWO, Side::Ask, 5, 1), None);
    assert_eq!(mine(&book), [3], "taken by a crossing order, gone from its owner too");
    assert_eq!(book.orders_of(user("0x3333333333333333333333333333333333333333")).count(), 0);
}

#[test]
fn levels_run_from_the_touch_outward_with_their_sizes_and_counts() {
    let mut book = Book::new();
    book.insert(Order { oid: 1, user: user(ONE), side: Side::Bid, px: dec("3244.50"), sz: dec("0.4") }, None);
    book.insert(Order { oid: 2, user: user(TWO), side: Side::Bid, px: dec("3245"), sz: dec("3.25") }, None);
    book.insert(Order { oid: 3, user: user(TWO), side: Side::Bid, px: dec("3244.5"), sz: dec("1.1") }, None);
    book.insert(Order { oid: 4, user: user(ONE), side: Side::Ask, px: dec("3247.5"), sz: dec("12") }, None);
    book.insert(Order { oid: 5, user: user(ONE), side: Side::Ask, px: dec("3246"), sz: dec("2") }, None);
    assert_eq!(
        book.levels(Side::Bid),
        [Level { px: dec("3245"), sz: dec("3.25"), orders: 1 }, Level { px: dec("3244.5"), sz: dec("1.5"), orders: 2 }]
    );
    assert_eq!(
        book.levels(Side::Ask),
        [Level { px: dec("3246"), sz: dec("2"), orders: 1 }, Level { px: dec("3247.5"), sz: dec("12"), orders: 1 }]
    );
    let bids: Vec<Order> = book.orders(Side::Bid).cloned().collect();
    assert_eq!(levels(&bids, Side::Bid), book.levels(Side::Bid), "a list of orders adds up the same");
    assert_eq!(book.orders(Side::Bid).map(|order| order.oid).collect::<Vec<_>>(), [2, 1, 3]);
}
