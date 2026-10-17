//! Live mainnet answers, 2026-10-08: `fundingHistory` for BTC from 15:00 UTC (three settlements, as
//! sent), `userFunding` for HLP Strategy A from 17:00 UTC (177 rows cut to BTC, ETH and DYDX),
//! `predictedFundings` (234 coins cut to four) and `perpsAtOpenInterestCap` (as sent).

use crate::account::ClearinghouseState;
use crate::markets::Dex;
use crate::markets::perps::funding::{
    FUNDING_INTERVAL_MS, FUNDING_PAGE, FUNDING_WINDOW_MS, FundingRate, HL_PERP, PredictedFundings, UserFunding,
    funding_history, funding_history_range, funding_history_request, funding_windows, merge_rates, next_funding,
    next_page, predicted_fundings, predicted_fundings_request, user_funding_range, user_funding_request,
};
use crate::markets::perps::meta::{Meta, perps_at_open_interest_cap, perps_at_open_interest_cap_request};
use crate::markets::perps::support::{Recorded, block_on, dec};
use crate::utils::Address;

const HISTORY: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/funding_history_btc_mainnet.json"));
const USER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/user_funding_mainnet.json"));
const PREDICTED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/predicted_fundings_mainnet.json"));
const AT_CAP: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/perps_at_open_interest_cap_mainnet.json"));
const META: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));

fn vault() -> Address {
    "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap()
}

fn history() -> Vec<FundingRate> {
    serde_json::from_str(HISTORY).unwrap()
}

fn user() -> Vec<UserFunding> {
    serde_json::from_str(USER).unwrap()
}

#[test]
fn settled_funding_reads_each_hour_s_rate_and_premium() {
    let history = history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].coin, "BTC");
    assert_eq!(history[0].funding_rate, dec("0.0000125"));
    assert_eq!(history[2].funding_rate, dec("0.000008154"));
    assert_eq!(history[1].premium, dec("-0.000413624"));
    assert_eq!(
        history.iter().map(|row| row.time).collect::<Vec<_>>(),
        [1_791_471_600_040, 1_791_475_200_042, 1_791_478_800_051]
    );
}

/// The venue stamps each settlement a few milliseconds past the hour, and the next one is an hour
/// on.
#[test]
fn funding_settles_on_the_hour() {
    let history = history();
    for pair in history.windows(2) {
        assert_eq!(next_funding(pair[0].time), pair[1].time - pair[1].time % FUNDING_INTERVAL_MS);
    }
    assert_eq!(next_funding(1_791_480_567_359), 1_791_482_400_000, "17:29 UTC settles at 18:00");
    assert_eq!(next_funding(1_791_478_800_000), 1_791_482_400_000, "on the hour, the next one");
}

/// What an account paid in an hour is at the rate `fundingHistory` lists for that hour: two reads
/// that agree. A positive rate has longs pay.
#[test]
fn an_account_s_funding_is_at_the_hour_s_settled_rate() {
    let rows = user();
    let btc = rows.iter().find(|row| row.delta.coin == "BTC").unwrap();
    let settled = history().into_iter().find(|row| row.time == btc.time).unwrap();
    assert_eq!(btc.delta.funding_rate, settled.funding_rate);
    assert_eq!((btc.delta.usdc, btc.delta.szi), (dec("-0.495632"), dec("0.75091")));
    assert_eq!(btc.delta.kind, "funding");
    assert_eq!(btc.delta.n_samples, None);
    assert_eq!(btc.hash, format!("0x{}", "0".repeat(64)), "every funding row's hash is zero");
    for row in &rows {
        let long = row.delta.szi.is_positive();
        let longs_pay = row.delta.funding_rate.is_positive();
        assert_eq!(row.delta.usdc.is_negative(), long == longs_pay, "{}", row.delta.coin);
    }
}

#[test]
fn predicted_funding_names_each_venue_and_its_interval() {
    let predicted: PredictedFundings = serde_json::from_str(PREDICTED).unwrap();
    let btc = predicted.get("BTC", HL_PERP).unwrap();
    assert_eq!(
        (btc.funding_rate, btc.next_funding_time, btc.funding_interval_hours),
        (dec("0.0000088332"), 1_791_478_800_000, Some(1))
    );
    assert_eq!(predicted.hyperliquid("BTC"), Some(btc));
    assert_eq!(btc.hourly(), Some(dec("0.0000088332")));
    let binance = predicted.get("BTC", "BinPerp").unwrap();
    assert_eq!(binance.hourly(), Some(dec("0.000002515")), "eight hours' rate over eight");
    assert_eq!(predicted.get("HYPE", "BinPerp"), None, "a venue that lists no such perp answers null");
    assert_eq!(predicted.venues("HYPE").map(<[_]>::len), Some(3));
    let ai = predicted.get("AI", "BinPerp").unwrap();
    assert_eq!((ai.funding_interval_hours, ai.hourly()), (None, None), "no interval, no hourly rate");
    assert_eq!(predicted.coins().collect::<Vec<_>>(), ["AI", "BTC", "ETH", "HYPE"]);
    assert_eq!(predicted.hyperliquid("SOL"), None);
}

#[test]
fn a_long_range_is_read_in_windows_of_twenty_days() {
    assert_eq!(FUNDING_WINDOW_MS, 480 * FUNDING_INTERVAL_MS);
    let start = 1_788_000_000_000;
    let end = start + 30 * 24 * FUNDING_INTERVAL_MS;
    assert_eq!(funding_windows(start, end), [(start, start + FUNDING_WINDOW_MS - 1), (start + FUNDING_WINDOW_MS, end)]);
    assert_eq!(funding_windows(start, start), [(start, start)]);
    assert_eq!(funding_windows(end, start), [], "an empty range reads nothing");
}

#[test]
fn windows_merge_into_one_history_inside_the_range() {
    let history = history();
    let first = history[..2].to_vec();
    let second = history[1..].to_vec();
    let merged = merge_rates(history[1].time, history[2].time, vec![second, first]);
    assert_eq!(merged, history[1..].to_vec(), "oldest first, once each, inside the range");
}

#[test]
fn a_full_page_goes_on_from_its_last_row() {
    let rows = user();
    assert_eq!(next_page(1_791_478_800_000, &rows), None, "a short page is the last");
    let full: Vec<UserFunding> = rows.iter().cycle().take(FUNDING_PAGE).cloned().collect();
    assert_eq!(next_page(1_791_478_800_000, &full), Some(1_791_478_800_051), "rows of other coins may share the time");
    assert_eq!(next_page(1_791_478_800_051, &full), Some(1_791_478_800_052), "never the same start twice");
}

#[test]
fn the_funding_reads_name_their_range() {
    assert_eq!(
        funding_history_request("BTC", 1_791_471_600_000, None),
        r#"{"type":"fundingHistory","coin":"BTC","startTime":1791471600000}"#
    );
    assert_eq!(
        funding_history_request("xyz:XYZ100", 1, Some(2)),
        r#"{"type":"fundingHistory","coin":"xyz:XYZ100","startTime":1,"endTime":2}"#
    );
    assert_eq!(
        user_funding_request(vault(), 1_791_478_800_000, None),
        r#"{"type":"userFunding","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a","startTime":1791478800000}"#
    );
    assert_eq!(predicted_fundings_request(), r#"{"type":"predictedFundings"}"#);
}

#[test]
fn a_range_reads_each_window_and_each_page_once() {
    let transport = Recorded::new(&[("fundingHistory", HISTORY), ("userFunding", USER)]);
    let one = block_on(funding_history(&transport, "BTC", 1_791_471_600_000, None)).unwrap();
    assert_eq!(one, history());
    // The first window, twenty days before, had no rows; the second is the recording.
    let transport = Recorded::new(&[("fundingHistory", "[]"), ("fundingHistory", HISTORY)]);
    let start = 1_791_471_600_000 - FUNDING_WINDOW_MS;
    let read = block_on(funding_history_range(&transport, "BTC", start, 1_791_478_800_051)).unwrap();
    assert_eq!(read, history());
    assert_eq!(transport.sent().len(), 2);
    let transport = Recorded::new(&[("userFunding", USER)]);
    let rows = block_on(user_funding_range(&transport, vault(), 1_791_478_800_000, 1_791_480_000_000, 20)).unwrap();
    assert_eq!(rows, user());
    assert_eq!(
        transport.sent(),
        [
            r#"{"type":"userFunding","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a","startTime":1791478800000,"endTime":1791480000000}"#
        ]
    );
    let transport = Recorded::new(&[("predictedFundings", PREDICTED)]);
    assert_eq!(block_on(predicted_fundings(&transport)).unwrap().coins().count(), 4);
}

/// The perps at their cap are main-dex perps `meta` lists (every one delisted the week before).
#[test]
fn the_perps_at_their_open_interest_cap_are_in_the_meta() {
    let at_cap: Vec<String> = serde_json::from_str(AT_CAP).unwrap();
    assert_eq!(at_cap, ["CANTO", "FTM", "JELLY", "LOOM", "RLB", "VINE", "ZEREBRO"]);
    let meta: Meta = serde_json::from_str(META).unwrap();
    assert!(at_cap.iter().all(|coin| meta.find(0, coin).is_some_and(|(_, perp)| perp.is_delisted)));
    assert_eq!(perps_at_open_interest_cap_request(Dex::Main), r#"{"type":"perpsAtOpenInterestCap"}"#);
    assert_eq!(
        perps_at_open_interest_cap_request(Dex::Hip3("xyz")),
        r#"{"type":"perpsAtOpenInterestCap","dex":"xyz"}"#
    );
    let transport = Recorded::new(&[("perpsAtOpenInterestCap", AT_CAP)]);
    assert_eq!(block_on(perps_at_open_interest_cap(&transport, Dex::Main)).unwrap(), at_cap);
}

/// `cumFunding` counts a payment as positive and `userFunding` as negative: HLP Strategy A's BTC
/// long, at a positive rate, has paid since it opened and paid this hour; its DYDX short received
/// both.
#[test]
fn cum_funding_and_user_funding_count_a_payment_with_opposite_signs() {
    let state: ClearinghouseState = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/perps/clearinghouse_hlp_a_mainnet.json"
    )))
    .unwrap();
    for coin in ["BTC", "DYDX"] {
        let since_open = state.position(coin).unwrap().cum_funding.as_ref().unwrap().since_open;
        let hour = user().into_iter().find(|row| row.delta.coin == coin).unwrap().delta.usdc;
        assert_eq!(since_open.is_positive(), hour.is_negative(), "{coin}");
    }
}
