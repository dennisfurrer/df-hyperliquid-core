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
fn the_tick_grid_is_exactly_what_the_venue_accepts() {
    for text in [
        "0.00001", "0.00002", "0.00009", "0.00025", "0.00333", "0.01", "0.0145", "0.15", "0.26999", "0.27", "0.27421",
        "0.44632", "0.9909", "0.95698", "0.99999",
    ] {
        let price = Px::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
        assert_eq!(price.to_wire(), text, "{text} round-trips");
        assert_eq!(price.to_dec(), dec(text), "{text}");
        assert_eq!(Px::from_dec(dec(text)), Ok(price), "{text}");
    }
    assert_eq!(Px::ONE.to_wire(), "1");
    assert_eq!(Px::ZERO.to_wire(), "0");
}

#[test]
fn a_price_off_the_grid_is_refused_rather_than_rounded() {
    assert_eq!(Px::parse("0.123456"), Err(BadPx::TooFine("0.123456".to_owned())));
    assert_eq!(Px::parse("0.2700000"), Ok(px("0.27")));
    assert!(matches!(Px::parse("1.5"), Err(BadPx::OutOfRange(_))));
    assert!(matches!(Px::parse("-0.5"), Err(BadPx::NotANumber(_))));
    assert!(matches!(Px::parse("abc"), Err(BadPx::NotANumber(_))));
    assert!(matches!(Px::parse(""), Err(BadPx::NotANumber(_))));
    assert_eq!(Px::from_ticks(100_001), None);
}

#[test]
fn a_side_and_its_complement_pay_one_unit() {
    let yes = px("0.27421");
    assert_eq!(yes.complement().to_wire(), "0.72579");
    assert_eq!(yes.complement().complement(), yes);
    assert_eq!(yes.checked_add(yes.complement()), Some(Px::ONE));
}

#[test]
fn stepping_stays_inside_the_range() {
    let price = px("0.5");
    assert_eq!(price.step(1).to_wire(), "0.50001");
    assert_eq!(price.step(-1).to_wire(), "0.49999");
    assert_eq!(Px::ZERO.step(-10), Px::ZERO);
    assert_eq!(Px::ONE.step(10), Px::ONE);
    assert_eq!(Px::ZERO.clamp_to_quotable(), Px::MIN_QUOTE);
    assert_eq!(Px::ONE.clamp_to_quotable(), Px::MAX_QUOTE);
    assert!(Px::MIN_QUOTE.is_quotable() && Px::MAX_QUOTE.is_quotable());
    assert!(!Px::ZERO.is_quotable() && !Px::ONE.is_quotable());
    assert_eq!((Px::MIN_QUOTE.to_dec(), Px::MAX_QUOTE.to_dec()), (dec("0.00001"), dec("0.99999")));
}

#[test]
fn a_decimal_goes_onto_the_grid_the_way_it_is_asked_to() {
    assert_eq!(Px::on_grid(dec("0.123456"), Round::Down), Some(px("0.12345")));
    assert_eq!(Px::on_grid(dec("0.123451"), Round::Up), Some(px("0.12346")));
    assert_eq!(Px::on_grid(dec("0.123455"), Round::Nearest), Some(px("0.12346")), "half away from zero");
    assert_eq!(Px::on_grid(dec("1.000001"), Round::Down), Some(Px::ONE));
    assert_eq!(Px::on_grid(dec("1.00001"), Round::Down), None, "outside the range");
    assert_eq!(Px::on_grid(dec("-0.000001"), Round::Up), None);
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
