//! Sizes as books print them, what they cost, and the fewest contracts worth the minimum.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_size_is_whole_contracts_and_says_so_when_it_is_not() {
    for (text, contracts) in [("544.0", 544), ("544", 544), ("+3", 3), (" 0 ", 0)] {
        let size = Qty::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
        assert_eq!(
            (size, size.to_wire(), size.to_string().parse()),
            (Qty::new(contracts), contracts.to_string(), Ok(size))
        );
    }
    let refused = [
        ("0.5", BadQty::Fractional("0.5".to_owned())),
        ("x", BadQty::NotAnInteger("x".to_owned())),
        ("-1", BadQty::NotAnInteger("-1".to_owned())),
        (".0", BadQty::NotAnInteger(".0".to_owned())),
        ("99999999999999999999", BadQty::NotAnInteger("99999999999999999999".to_owned())),
    ];
    for (text, why) in refused {
        assert_eq!(Qty::parse(text), Err(why), "{text}");
    }
    assert!(serde_json::from_str::<Qty>(r#""0.5""#).is_err());
    assert_eq!(serde_json::from_str::<Qty>(r#""544.0""#).unwrap(), Qty::new(544));
    assert_eq!((Qty::from_dec(dec("50")), Qty::new(50).to_dec()), (Ok(Qty::new(50)), Dec::from_int(50)));
    assert_eq!(Qty::from_dec(dec("50.5")), Err(BadQty::Fractional("50.5".to_owned())));
    assert_eq!(Qty::from_dec(dec("-2")), Err(BadQty::NotAnInteger("-2".to_owned())));
    assert_eq!((Qty::new(5).min(Qty::new(3)), Qty::new(3).saturating_sub(Qty::new(5))), (Qty::new(3), Qty::ZERO));
}

#[test]
fn contracts_cost_their_size_times_the_price_and_an_amount_buys_whole_ones_toward_zero() {
    let notional = [
        (544, "0.27", "146.88"),
        (34, "0.21349", "7.25866"),
        (1, "1", "1"),
        (0, "0.27", "0"),
        (u64::MAX, "1", "18446744073709551615"),
    ];
    for (contracts, price, cost) in notional {
        assert_eq!(Qty::new(contracts).notional(px(price)), dec(cost), "{contracts} at {price}");
    }
    let spend = [
        ("10", "0.612", Some(16)),
        ("10", "0.5", Some(20)),
        ("0.4", "0.5", Some(0)),
        ("10", "0", None),
        ("-1", "0.5", None),
    ];
    for (amount, price, contracts) in spend {
        assert_eq!(Qty::spend(dec(amount), px(price)), contracts.map(Qty::new), "{amount} at {price}");
    }
}

/// An order worth under 10 USDC at its own price, the spot minimum, is refused by the venue.
#[test]
fn the_ten_dollar_minimum_tracks_the_price_and_not_the_collateral() {
    let least = [
        ("0.99697", Some(11)),
        ("0.59666", Some(17)),
        ("0.5", Some(20)),
        ("0.01", Some(1_000)),
        ("1", Some(10)),
        ("0", None),
    ];
    for (price, contracts) in least {
        let at = px(price);
        assert_eq!(Qty::least_at(at), contracts.map(Qty::new), "{price}");
        if let Some(contracts) = contracts {
            assert!(Qty::new(contracts).worth_minimum(at) && !Qty::new(contracts - 1).worth_minimum(at), "{price}");
        }
    }
}
