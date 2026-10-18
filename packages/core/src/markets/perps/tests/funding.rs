//! Live mainnet answers, 2026-10-08: `fundingHistory` for BTC from 15:00 UTC (three settlements, as
//! sent), `userFunding` for HLP Strategy A from 17:00 UTC (177 rows cut to BTC, ETH and DYDX),
//! `predictedFundings` (234 coins cut to four) and `perpsAtOpenInterestCap` (as sent). Reads
//! through a client, windows and pages included, are in `tests/mocked/markets`.

use crate::account::ClearinghouseState;
use crate::markets::perps::funding::{
    FUNDING_INTERVAL_MS, FUNDING_PAGE, FUNDING_WINDOW_MS, FundingRate, HL_PERP, PredictedFundings, UserFunding,
    funding_windows, merge_rates, next_funding, next_page,
};
use crate::markets::perps::meta::Meta;
use crate::markets::perps::support::dec;

const HISTORY: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/funding_history_btc_mainnet.json"));
const USER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/user_funding_mainnet.json"));
const PREDICTED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/predicted_fundings_mainnet.json"));
const AT_CAP: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/perps_at_open_interest_cap_mainnet.json"));
const META: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));
const HLP_A: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_hlp_a_mainnet.json"));

fn history() -> Vec<FundingRate> {
    serde_json::from_str(HISTORY).unwrap()
}

fn user() -> Vec<UserFunding> {
    serde_json::from_str(USER).unwrap()
}

/// The venue stamps each settlement a few milliseconds past the hour, and the next one is an hour
/// on.
#[test]
fn funding_settles_each_hour_at_a_rate_and_a_premium() {
    let history = history();
    let read: Vec<(u64, &str, &str)> = vec![
        (1_791_471_600_040, "0.0000125", "-0.0002226288"),
        (1_791_475_200_042, "0.000010797", "-0.000413624"),
        (1_791_478_800_051, "0.000008154", "-0.0004347679"),
    ];
    for (row, (time, rate, premium)) in history.iter().zip(read) {
        assert_eq!(
            (row.coin.as_str(), row.time, row.funding_rate, row.premium),
            ("BTC", time, dec(rate), dec(premium))
        );
    }
    for pair in history.windows(2) {
        assert_eq!(next_funding(pair[0].time), pair[1].time - pair[1].time % FUNDING_INTERVAL_MS);
    }
    for (now, next) in [(1_791_480_567_359, 1_791_482_400_000), (1_791_478_800_000, 1_791_482_400_000), (0, 3_600_000)]
    {
        assert_eq!(next_funding(now), next, "{now}: the next whole hour");
    }
}

/// What an account paid in an hour is at the rate `fundingHistory` lists for that hour, and
/// `cumFunding` counts a payment as positive where `userFunding` counts it negative: HLP Strategy
/// A's BTC long, at a positive rate, has paid since it opened and paid this hour; its DYDX short
/// received both.
#[test]
fn an_account_s_funding_is_at_the_hour_s_settled_rate_and_longs_pay_a_positive_one() {
    let rows = user();
    let btc = &rows[0];
    let settled = history().into_iter().find(|row| row.time == btc.time).unwrap();
    assert_eq!((btc.delta.coin.as_str(), btc.delta.funding_rate), ("BTC", settled.funding_rate));
    assert_eq!((btc.delta.kind.as_str(), btc.delta.usdc, btc.delta.szi), ("funding", dec("-0.495632"), dec("0.75091")));
    assert_eq!((btc.delta.n_samples, btc.hash.clone()), (None, format!("0x{}", "0".repeat(64))), "every hash is zero");
    for row in &rows {
        let longs_pay = row.delta.funding_rate.is_positive();
        assert_eq!(row.delta.usdc.is_negative(), row.delta.szi.is_positive() == longs_pay, "{}", row.delta.coin);
    }
    let state: ClearinghouseState = serde_json::from_str(HLP_A).unwrap();
    for coin in ["BTC", "DYDX"] {
        let since_open = state.position(coin).unwrap().cum_funding.as_ref().unwrap().since_open;
        let hour = rows.iter().find(|row| row.delta.coin == coin).unwrap().delta.usdc;
        assert_eq!(since_open.is_positive(), hour.is_negative(), "{coin}");
    }
}

#[test]
fn predicted_funding_names_each_venue_and_its_interval() {
    let predicted: PredictedFundings = serde_json::from_str(PREDICTED).unwrap();
    assert_eq!(predicted.coins().collect::<Vec<_>>(), ["AI", "BTC", "ETH", "HYPE"]);
    let btc = predicted.hyperliquid("BTC").unwrap();
    assert_eq!(
        (btc.funding_rate, btc.next_funding_time, btc.funding_interval_hours),
        (dec("0.0000088332"), 1_791_478_800_000, Some(1))
    );
    let cases = [
        ("BTC", HL_PERP, Some(dec("0.0000088332"))),
        ("BTC", "BinPerp", Some(dec("0.000002515"))),
        ("AI", "BinPerp", None),
    ];
    for (coin, venue, hourly) in cases {
        assert_eq!(predicted.get(coin, venue).unwrap().hourly(), hourly, "{coin} on {venue}: the rate over its hours");
    }
    assert_eq!(predicted.get("HYPE", "BinPerp"), None, "a venue that lists no such perp answers null");
    assert_eq!(predicted.venues("HYPE").map(<[_]>::len), Some(3));
    assert_eq!((predicted.hyperliquid("SOL"), predicted.venues("SOL")), (None, None));
}

/// A long range is read in windows of twenty days, under the 500 rows a read answers at most, and
/// the windows merged: inside the range, oldest first, each settlement once.
#[test]
fn a_long_range_is_read_in_windows_and_merged_into_one_history() {
    assert_eq!(FUNDING_WINDOW_MS, 480 * FUNDING_INTERVAL_MS);
    let (start, day) = (1_788_000_000_000, 24 * FUNDING_INTERVAL_MS);
    let windows = [
        (
            start,
            start + 30 * day,
            vec![(start, start + FUNDING_WINDOW_MS - 1), (start + FUNDING_WINDOW_MS, start + 30 * day)],
        ),
        (start, start + 20 * day - 1, vec![(start, start + 20 * day - 1)]),
        (start, start, vec![(start, start)]),
        (start + 1, start, vec![]),
        (u64::MAX - 1, u64::MAX, vec![(u64::MAX - 1, u64::MAX)]),
    ];
    for (from, to, cut) in windows {
        assert_eq!(funding_windows(from, to), cut, "{from} to {to}");
    }
    let history = history();
    let merged = merge_rates(history[1].time, history[2].time, vec![history[1..].to_vec(), history[..2].to_vec()]);
    assert_eq!(merged, history[1..].to_vec());
    assert_eq!(merge_rates(0, u64::MAX, vec![Vec::new(), history.clone()]), history);
}

/// A full page resumes at its last row's time, since other coins' rows may share it, or a
/// millisecond later if that would read the same page again.
#[test]
fn a_full_page_goes_on_from_its_last_row_and_a_short_one_is_the_last() {
    let rows = user();
    let full: Vec<UserFunding> = rows.iter().cycle().take(FUNDING_PAGE).cloned().collect();
    let pages = [
        (1_791_478_800_000, &rows, None),
        (1_791_478_800_000, &full, Some(1_791_478_800_051)),
        (1_791_478_800_051, &full, Some(1_791_478_800_052)),
    ];
    for (start, page, next) in pages {
        assert_eq!(next_page(start, page), next, "{start}, {} rows", page.len());
    }
}

/// The perps at their cap are main-dex perps `meta` lists, every one delisted the week before.
#[test]
fn the_perps_at_their_open_interest_cap_are_in_the_meta() {
    let at_cap: Vec<String> = serde_json::from_str(AT_CAP).unwrap();
    assert_eq!(at_cap, ["CANTO", "FTM", "JELLY", "LOOM", "RLB", "VINE", "ZEREBRO"]);
    let meta: Meta = serde_json::from_str(META).unwrap();
    assert!(at_cap.iter().all(|coin| meta.find(0, coin).is_some_and(|(_, perp)| perp.is_delisted)));
}
