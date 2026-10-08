//! hip4-tui's `core/src/tests/px.rs`, prices seen on live outcome books, and
//! atlas's `hip4_pricing.json` (hip4-sdk 2.0.0 `pricing.ts`, its minimum
//! shares exact by decimal.js).

use hl_core::Json;

use super::*;
use crate::coin::{MAX_PRICE, MIN_PRICE};

const PRICING: &str = include_str!("../../../../vectors/hip4_pricing.json");

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
    assert_eq!((Px::MIN_QUOTE.to_dec(), Px::MAX_QUOTE.to_dec()), (MIN_PRICE, MAX_PRICE));
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

#[test]
fn a_mid_is_not_an_orderable_price() {
    let mid = Mid::parse("0.270145").unwrap();
    assert_eq!(mid.to_wire(), "0.270145");
    assert!(!mid.is_orderable());
    assert!(Px::parse("0.270145").is_err());
    assert_eq!(mid.to_px_floor().to_wire(), "0.27014");
    assert_eq!(mid.to_px_ceil().to_wire(), "0.27015");
    assert_eq!(mid.to_px_nearest().to_wire(), "0.27015");
    assert_eq!(mid.to_dec(), dec("0.270145"));
    assert!(matches!(Mid::parse("0.123456789"), Err(BadPx::TooFine(_))));
}

#[test]
fn every_orderable_price_is_an_observable_one() {
    for text in ["0.00001", "0.27", "0.26999", "0.5", "0.99999"] {
        let price = px(text);
        let mid = Mid::of(price);
        assert!(mid.is_orderable(), "{text}");
        assert_eq!(mid.to_px_nearest(), price, "{text}");
        assert_eq!(mid.to_wire(), text, "{text}");
    }
    assert_eq!(Mid::of(Px::ONE), Mid::ONE);
    assert_eq!(Mid::ONE.complement(), Mid::ZERO);
    assert_eq!(Mid::parse("0.126535").unwrap().complement().to_wire(), "0.873465");
}

#[test]
fn sizes_are_whole_contracts_and_say_so_when_they_are_not() {
    assert_eq!(Qty::parse("544.0"), Ok(Qty::new(544)), "as a book prints it");
    assert_eq!(Qty::parse("544").unwrap().to_wire(), "544");
    assert_eq!(Qty::parse("0.5"), Err(BadQty::Fractional("0.5".to_owned())));
    assert!(matches!(Qty::parse("x"), Err(BadQty::NotAnInteger(_))));
    assert!(matches!(Qty::parse("-1"), Err(BadQty::NotAnInteger(_))));
    assert_eq!(Qty::new(50).to_dec(), Dec::from_int(50));
    assert_eq!(Qty::from_dec(dec("50")), Ok(Qty::new(50)));
    assert_eq!(Qty::from_dec(dec("50.5")), Err(BadQty::Fractional("50.5".to_owned())));
}

#[test]
fn a_holding_may_be_short_and_still_reads_like_a_size() {
    assert_eq!(SignedQty::parse("1355.0"), Ok(SignedQty::new(1355)));
    assert_eq!(SignedQty::parse("-539.0"), Ok(SignedQty::new(-539)));
    assert_eq!(SignedQty::parse("0.0"), Ok(SignedQty::ZERO));
    assert_eq!(SignedQty::parse(" -7 ").unwrap().to_wire(), "-7");
    assert!(matches!(SignedQty::parse("-0.5"), Err(BadSignedQty::Size(BadQty::Fractional(_)))));
    assert!(matches!(SignedQty::parse("x"), Err(BadSignedQty::Size(BadQty::NotAnInteger(_)))));
    assert!(matches!(SignedQty::parse("-10000000000000000000"), Err(BadSignedQty::TooLarge(_))));
}

#[test]
fn a_short_leg_is_a_magnitude_and_a_sign_when_asked() {
    let short = SignedQty::short(Qty::new(539));
    let long = SignedQty::long(Qty::new(539));
    assert_eq!(short.abs(), Qty::new(539));
    assert_eq!(long.abs(), short.abs());
    assert!(short.is_short() && !long.is_short());
    assert_eq!(long.saturating_add(short), SignedQty::ZERO);
    assert!(short < long, "holdings order numerically, not by magnitude");
    assert_eq!(SignedQty::new(i64::MIN).abs().get(), 1u64 << 63);
    assert_eq!(SignedQty::new(i64::MAX).saturating_add(long), SignedQty::new(i64::MAX));
}

#[test]
fn a_notional_is_the_size_times_the_price_in_quote_token() {
    assert_eq!(Qty::new(544).notional(px("0.27")), dec("146.88"));
    assert_eq!(Qty::new(34).notional(px("0.21349")), dec("7.25866"), "a live fill, 2026-09-24");
    assert_eq!(Qty::new(1).notional(Px::ONE), Dec::ONE);
    assert_eq!(Qty::new(0).notional(px("0.27")), Dec::ZERO);
    assert_eq!(Qty::new(u64::MAX).notional(Px::ONE), Dec::new(i128::from(u64::MAX), 0).unwrap(), "never wraps");
}

/// hip4-tui's rule: an order worth under 10 USDC at its own price is refused
/// before a nonce is drawn, the spot minimum.
#[test]
fn the_ten_dollar_minimum_tracks_the_price_and_not_the_collateral() {
    let near_one = px("0.99697");
    assert_eq!(Qty::least_at(near_one), Some(Qty::new(11)));
    assert!(Qty::new(11).worth_minimum(near_one));
    assert!(!Qty::new(10).worth_minimum(near_one));
    assert_eq!(Qty::least_at(px("0.5")), Some(Qty::new(20)));
    assert_eq!(Qty::least_at(px("0.01")), Some(Qty::new(1_000)));
    assert_eq!(Qty::least_at(Px::ONE), Some(Qty::new(10)));
    assert_eq!(Qty::least_at(Px::ZERO), None, "nothing at zero is worth anything");
}

#[test]
fn an_amount_of_quote_buys_whole_contracts_toward_zero() {
    assert_eq!(Qty::spend(dec("10"), px("0.612")), Some(Qty::new(16)), "atlas's ten dollars of YES");
    assert_eq!(Qty::spend(dec("10"), px("0.5")), Some(Qty::new(20)));
    assert_eq!(Qty::spend(dec("0.4"), px("0.5")), Some(Qty::ZERO));
    assert_eq!(Qty::spend(dec("10"), Px::ZERO), None);
    assert_eq!(Qty::spend(dec("-1"), px("0.5")), None);
}

/// hip4-sdk's `formatPrice` keeps five significant figures at a tick of
/// `10^(floor(log10 p) - 4)`, which is finer than the venue's grid below
/// 0.1. Where its tick is the grid's, the nearest grid price is its answer;
/// where it is finer, its answer is no price the venue takes (D1), as
/// atlas's `OUTCOME_MAX_DECIMALS` also holds: 0.012346 has no NO-side twin on
/// a five-place book.
#[test]
fn hip4_sdk_prices_agree_where_its_tick_is_the_venue_s() {
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

/// `getMinShares`: `ceil(1.5 / clamp(min(p, 1 - p), 0.01, 1))`, exact. The
/// file keeps the TypeScript's float answer where it differs (0.9 gives 16
/// there, since `1 - 0.9` is not 0.1 in a float); the exact one is checked.
#[test]
fn the_minimum_shares_are_hip4_sdk_s_rule_exactly() {
    let file = Json::parse(PRICING).unwrap();
    assert_eq!(file.get("minNotional").and_then(Json::as_str).map(dec), Some(MIN_NOTIONAL));
    let cases = file.get("minShares").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 19);
    for case in cases {
        let mark = case.get("mark").and_then(Json::as_str).unwrap();
        let shares = case.get("minShares").and_then(Json::as_u64).unwrap();
        assert_eq!(min_shares(dec(mark)), Qty::new(shares), "{mark}");
    }
    assert_eq!(min_shares(dec("0.9")), Qty::new(15));
}
