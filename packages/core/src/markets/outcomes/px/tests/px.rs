//! Prices seen on live outcome books, and price grid vectors from a TypeScript implementation.

use crate::utils::Json;

use super::*;

const PRICING: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/hip4_pricing.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_price_reads_exactly_on_the_grid_and_is_refused_off_it() {
    for text in [
        "0", "0.00001", "0.00002", "0.00009", "0.00025", "0.00333", "0.01", "0.0145", "0.15", "0.26999", "0.27",
        "0.27421", "0.44632", "0.9909", "0.95698", "0.99999", "1",
    ] {
        let price = Px::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
        assert_eq!((price.to_wire(), price.to_string().parse::<Px>()), (text.to_owned(), Ok(price)), "{text}");
        assert_eq!((price.to_dec(), Px::from_dec(dec(text))), (dec(text), Ok(price)), "{text}");
        assert_eq!(serde_json::from_str::<Px>(&format!("\"{text}\"")).unwrap(), price);
    }
    assert_eq!(px("0.2700000"), px("0.27"), "trailing zeros are no finer");
    let refused = [
        ("0.123456", BadPx::TooFine("0.123456".to_owned())),
        ("1.5", BadPx::OutOfRange("1.5".to_owned())),
        ("1.00001", BadPx::OutOfRange("1.00001".to_owned())),
        ("-0.5", BadPx::NotANumber("-0.5".to_owned())),
        ("abc", BadPx::NotANumber("abc".to_owned())),
        ("", BadPx::NotANumber(String::new())),
    ];
    for (text, why) in refused {
        assert_eq!(Px::parse(text), Err(why), "{text:?}");
    }
    assert_eq!(Px::from_dec(dec("0.270001")), Err(BadPx::TooFine("0.270001".to_owned())));
    assert!(serde_json::from_str::<Px>(r#""0.123456""#).is_err());
    assert_eq!((Px::from_ticks(100_000), Px::from_ticks(100_001)), (Some(Px::ONE), None));
}

#[test]
fn a_price_moves_along_the_grid_and_stays_inside_the_range() {
    let yes = px("0.27421");
    assert_eq!((yes.ticks(), yes.complement().to_wire()), (27_421, "0.72579".to_owned()));
    assert_eq!(yes.complement().complement(), yes);
    assert_eq!(yes.checked_add(yes.complement()), Some(Px::ONE), "a side and its complement pay one unit");
    assert_eq!(Px::ONE.checked_add(Px::MIN_QUOTE), None);
    assert_eq!((yes.midpoint(px("0.27424")), yes.abs_diff(px("0.27424"))), (px("0.27422"), 3), "down to a tick");
    for (from, ticks, to) in [("0.5", 1, "0.50001"), ("0.5", -1, "0.49999"), ("0", -10, "0"), ("1", 10, "1")] {
        assert_eq!(px(from).step(ticks), px(to), "{from} by {ticks}");
    }
    for (price, quotable, clamped) in [("0", false, "0.00001"), ("0.5", true, "0.5"), ("1", false, "0.99999")] {
        assert_eq!((px(price).is_quotable(), px(price).clamp_to_quotable()), (quotable, px(clamped)), "{price}");
    }
    assert_eq!((Px::MIN_QUOTE.to_dec(), Px::MAX_QUOTE.to_dec()), (dec("0.00001"), dec("0.99999")));
    assert!(Px::ZERO.is_zero() && !Px::MIN_QUOTE.is_zero());
}

#[test]
fn a_decimal_goes_onto_the_grid_the_way_it_is_asked_to() {
    let cases = [
        ("0.123456", Round::Down, Some("0.12345")),
        ("0.123451", Round::Up, Some("0.12346")),
        ("0.123455", Round::Nearest, Some("0.12346")),
        ("1.000001", Round::Down, Some("1")),
        ("1.00001", Round::Down, None),
        ("-0.000001", Round::Up, None),
    ];
    for (value, round, price) in cases {
        assert_eq!(Px::on_grid(dec(value), round), price.map(px), "{value} {round:?}");
    }
    assert_eq!(Px::nearest(dec("0.999995")), Some(Px::ONE), "a price, not a quotable one");
}

/// The vectors keep five significant figures at a tick of `10^(floor(log10 p) - 4)`, finer than
/// the venue's grid below 0.1. Where that tick is the grid's, the nearest grid price agrees; where
/// it is finer, the figure is no price the venue takes: 0.012346 has no No-side twin on a
/// five-place book.
#[test]
fn reference_prices_agree_where_its_tick_is_the_venue_s() {
    let file = Json::parse(PRICING).unwrap();
    let cases = file.get("prices").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 23);
    let (mut agreed, mut finer) = (0, 0);
    for case in cases {
        let text = |key: &str| case.get(key).and_then(Json::as_str).unwrap();
        let (input, tick, price) = (text("input"), text("tick"), text("price"));
        match Px::parse(price) {
            Ok(on_grid) => {
                assert_eq!(Px::nearest(dec(input)), Some(on_grid), "{input}");
                agreed += 1;
            }
            Err(error) => {
                assert_eq!(error, BadPx::TooFine(price.to_owned()), "{input}");
                assert_ne!(tick, "0.00001", "{input}: a finer answer comes only from a finer tick");
                assert!(Px::nearest(dec(input)).is_some(), "{input} still has a nearest grid price");
                finer += 1;
            }
        }
        if tick == "0.00001" {
            assert_eq!(Px::from_ticks(1).unwrap().to_dec(), dec(tick));
        }
    }
    assert_eq!((agreed, finer), (18, 5));
}
