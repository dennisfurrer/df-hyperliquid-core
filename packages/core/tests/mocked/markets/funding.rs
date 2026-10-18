//! Funding over more than one read: settled rates in twenty-day windows, merged, and an account's
//! payments page by page.

use df_hyperliquid_core::markets::perps::{self, FUNDING_PAGE, UserFunding};
use df_hyperliquid_core::utils::Address;
use df_hyperliquid_core::{Client, Network};
use serde_json::Value;

use super::{bodies, dec};
use crate::support::venue::Venue;
use crate::support::{clock, fixture};

const HOUR: u64 = 3_600_000;
/// 16:00 UTC on 2026-10-08, between the recording's first settlement and its second.
const SIXTEEN: u64 = 1_791_475_200_000;
/// The recording's last settlement, 17:00 UTC.
const LAST: u64 = 1_791_478_800_051;

/// HLP Strategy A, whose `userFunding` is recorded.
fn vault() -> Address {
    "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap()
}

fn rows(path: &str) -> Vec<Value> {
    serde_json::from_str(&fixture(path)).unwrap()
}

/// Forty days to 17:00 read as two windows of twenty days and one to the end, the boundary at
/// 16:00 cutting the recording's three settlements one and two; then the day's read with no end,
/// and the next settlement predicted.
#[tokio::test]
async fn btc_funding_over_forty_days_is_read_in_windows_and_merged_into_one_history() {
    let settled = rows("perps/funding_history_btc_mainnet.json");
    let window = perps::FUNDING_WINDOW_MS;
    let start = SIXTEEN - 2 * window;
    let answer = |rows: &[Value]| Value::Array(rows.to_vec()).to_string();
    let venue = Venue::mainnet()
        .info_where("fundingHistory", &[("startTime", &start.to_string())], "[]")
        .info_where("fundingHistory", &[("startTime", &(SIXTEEN - window).to_string())], &answer(&settled[..1]))
        .info_where("fundingHistory", &[("startTime", &SIXTEEN.to_string())], &answer(&settled[1..]))
        .info("fundingHistory", "perps/funding_history_btc_mainnet.json")
        .info("predictedFundings", "perps/predicted_fundings_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let history = perps::funding_history_range(&client, "BTC", start, LAST).await.unwrap();
    let today = perps::funding_history(&client, "BTC", LAST - 2 * HOUR - 51, None).await.unwrap();
    let predicted = perps::predicted_fundings(&client).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"fundingHistory","coin":"BTC","startTime":1788019200000,"endTime":1789747199999}"#,
            r#"{"type":"fundingHistory","coin":"BTC","startTime":1789747200000,"endTime":1791475199999}"#,
            r#"{"type":"fundingHistory","coin":"BTC","startTime":1791475200000,"endTime":1791478800051}"#,
            r#"{"type":"fundingHistory","coin":"BTC","startTime":1791471600000}"#,
            r#"{"type":"predictedFundings"}"#,
        ]
    );
    let times: Vec<u64> = history.iter().map(|rate| rate.time).collect();
    assert_eq!(times, [1_791_471_600_040, 1_791_475_200_042, LAST], "oldest first, each once");
    assert_eq!(history, today, "the windows merged are the day read whole");
    assert_eq!((history[0].funding_rate, history[2].funding_rate), (dec("0.0000125"), dec("0.000008154")));
    let next = predicted.hyperliquid("BTC").unwrap();
    assert_eq!(
        (next.next_funding_time, next.hourly()),
        (LAST - 51, Some(dec("0.0000088332"))),
        "17:00, an hour's rate"
    );
}

/// A full page of 500 rows, three coins an hour ending part way through 17:00, goes on from 17:00:
/// the second page repeats that hour's rows and adds the one the first left out.
#[tokio::test]
async fn an_account_s_funding_is_paged_until_a_page_comes_back_short() {
    let hour = rows("perps/user_funding_mainnet.json");
    let first = LAST - 166 * HOUR;
    let page: Vec<Value> = (0..167)
        .flat_map(|at| {
            hour.iter().map(move |row| {
                let mut row = row.clone();
                row["time"] = Value::from(first + at * HOUR);
                row
            })
        })
        .take(FUNDING_PAGE)
        .collect();
    let start = first - 51;
    let end = LAST + 1_199_949;
    let venue = Venue::mainnet()
        .info_where("userFunding", &[("startTime", &start.to_string())], &Value::Array(page).to_string())
        .info_where("userFunding", &[("startTime", &LAST.to_string())], &fixture("perps/user_funding_mainnet.json"));
    let client = Client::reader(venue, Network::Mainnet, clock);

    let paid = perps::user_funding_range(&client, vault(), start, end, 10).await.unwrap();

    let user = r#""user":"0x010461c14e146ac35fe42271bdc1134ee31c703a""#;
    assert_eq!(
        bodies(&client),
        [
            format!(r#"{{"type":"userFunding",{user},"startTime":1790881200000,"endTime":1791480000000}}"#),
            format!(r#"{{"type":"userFunding",{user},"startTime":1791478800051,"endTime":1791480000000}}"#),
        ]
    );
    assert_eq!(paid.len(), FUNDING_PAGE + 1);
    let last_hour: Vec<(u64, &str)> =
        paid[FUNDING_PAGE - 2..].iter().map(|row: &UserFunding| (row.time, row.delta.coin.as_str())).collect();
    assert_eq!(last_hour, [(LAST, "BTC"), (LAST, "ETH"), (LAST, "DYDX")], "each position's settlement once");
    assert!(paid.windows(2).all(|pair| pair[0].time <= pair[1].time), "oldest first");
    assert_eq!(paid[FUNDING_PAGE].delta.usdc, dec("0.44349"));
}
