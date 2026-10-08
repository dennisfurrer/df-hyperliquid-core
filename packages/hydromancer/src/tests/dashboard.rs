//! The outcome builder dashboard. The stats, category and hourly answers are live mainnet
//! recordings (hip4-tui, 2026-09-18); the daily and profitability answers are Hydromancer's
//! documented examples. Where two answers describe the same builder, each is checked against the
//! other: a category's share is its count over the builder's total, to the tenth place.

use hl_core::Round;

use super::*;

const BUILDER: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";
const STATS: &str = include_str!("fixtures/dashboard_stats.json");
const CATEGORIES: &str = include_str!("fixtures/dashboard_categories.json");
const HOURLY: &str = include_str!("fixtures/dashboard_hourly.json");
const DAILY: &str = include_str!("fixtures/dashboard_volume_daily.json");
const PROFITABILITY: &str = include_str!("fixtures/dashboard_profitability.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn builder() -> Address {
    BUILDER.parse().unwrap()
}

/// `part / whole` to `places`, rounded half away from zero, as the provider rounds a share.
fn share(part: Dec, whole: Dec, places: u32) -> Dec {
    part.checked_div(whole, places, Round::Nearest).unwrap()
}

#[test]
fn all_time_stats_read_as_exact_decimals() {
    let stats: BuilderStats = serde_json::from_str(STATS).unwrap();
    assert_eq!(stats.builder, Some(builder()));
    assert_eq!(stats.builder_fee, dec("40.73"));
    assert_eq!(stats.volume, dec("72078742.99"));
    assert_eq!(stats.volume_share, dec("0.1691"));
    assert_eq!((stats.fills, stats.unique_users), (1_347_844, 4_283));
}

#[test]
fn the_bucket_with_no_builder_reads_as_none_however_it_is_spelled() {
    for said in [r#"null"#, r#""null""#, r#""NULL""#] {
        let text = format!(
            r#"{{"builder":{said},"builderFee":"0.00","volume":"1","volumeShare":"0.77","fills":1,"uniqueUsers":1}}"#
        );
        let stats: BuilderStats = serde_json::from_str(&text).unwrap();
        assert_eq!(stats.builder, None, "{said}");
    }
    assert!(serde_json::from_str::<BuilderStats>(&STATS.replace(BUILDER, "0xabc...123")).is_err());
}

#[test]
fn a_category_share_is_its_count_over_the_builder_s_total() {
    let stats: BuilderStats = serde_json::from_str(STATS).unwrap();
    let split: CategoryStats = serde_json::from_str(CATEGORIES).unwrap();
    assert_eq!(split.builder, Some(builder()));
    let overall = split.overall.as_ref().unwrap();
    assert_eq!((overall.trading_days, overall.unique_markets), (139, 2772));
    assert_eq!((overall.max_markets_day.date.as_str(), overall.max_markets_day.markets), ("2026-09-16", 289));
    assert_eq!(split.categories.len(), 2);
    for category in &split.categories {
        let fills = Dec::from_int(i64::try_from(category.fills).unwrap());
        let users = Dec::from_int(i64::try_from(category.unique_users).unwrap());
        let all_fills = Dec::from_int(i64::try_from(stats.fills).unwrap());
        let all_users = Dec::from_int(i64::try_from(stats.unique_users).unwrap());
        assert_eq!(category.fills_share, Some(share(fills, all_fills, 10)), "{}", category.category);
        assert_eq!(category.unique_users_share, Some(share(users, all_users, 10)), "{}", category.category);
        assert_eq!(category.volume_share, share(category.volume, stats.volume, 10), "{}", category.category);
    }
    let sports = &split.categories[1];
    assert_eq!((sports.category.as_str(), sports.builder_fee, sports.trading_days), ("sports", Dec::ZERO, Some(56)));
    assert_eq!(sports.max_markets_day.as_ref().map(|day| day.markets), Some(69));
}

#[test]
fn an_hourly_answer_carries_one_figure_an_hour_on_every_series() {
    let hourly: Hourly = serde_json::from_str(HOURLY).unwrap();
    assert!(hourly.is_aligned());
    assert_eq!(hourly.hours.len(), 3);
    assert_eq!(hourly.start_hour.as_deref(), Some("2026-09-17 18:00:00"));
    assert_eq!(hourly.series.volume[2], dec("40973.04335"));
    assert_eq!((hourly.series.fills[0], hourly.series.active_users[1], hourly.series.new_users[2]), (3497, 92, 0));
    assert_eq!(hourly.series.biggest_notional_fill[1], dec("2895.9942"));
}

#[test]
fn a_short_hourly_series_is_not_drawn_as_a_quiet_hour() {
    let mut hourly: Hourly = serde_json::from_str(HOURLY).unwrap();
    hourly.series.volume.pop();
    assert!(!hourly.is_aligned());
    let mut hourly: Hourly = serde_json::from_str(HOURLY).unwrap();
    hourly.series.biggest_notional_fill.pop();
    assert!(!hourly.is_aligned(), "a series the answer carries must cover every hour");
}

#[test]
fn a_packed_daily_answer_keeps_both_axes_and_the_bucket_with_no_builder() {
    let daily: VolumeDaily = serde_json::from_str(DAILY).unwrap();
    assert_eq!((daily.start_date.as_str(), daily.end_date.as_str()), ("2026-06-01", "2026-06-03"));
    assert_eq!(daily.builders, vec![Some(builder()), None]);
    assert_eq!(daily.dates.len(), 3);
    assert_eq!(daily.series.volume[0][2], dec("143000"));
    assert_eq!(daily.series.builder_fee[1][0], dec("100"));
    assert_eq!((daily.series.fills[1][1], daily.series.unique_users[0][2]), (402, 165));
    assert!(daily.is_aligned());
}

#[test]
fn a_ragged_daily_answer_is_refused_rather_than_folded_short() {
    let mut daily: VolumeDaily = serde_json::from_str(DAILY).unwrap();
    daily.series.volume[1].pop();
    assert!(!daily.is_aligned(), "a short row under-reports its builder");
    let mut daily: VolumeDaily = serde_json::from_str(DAILY).unwrap();
    daily.series.fills.pop();
    assert!(!daily.is_aligned(), "a missing row is not an empty one");
}

#[test]
fn profitability_adds_up_the_way_its_page_says() {
    let read: Profitability = serde_json::from_str(PROFITABILITY).unwrap();
    assert_eq!(read.builder, Some(builder()));
    assert_eq!((read.window.as_str(), read.scope.as_str(), read.days), ("30d", "outcome", 30));
    assert_eq!(read.wins + read.losses, read.trades, "an even trade counts as a loss");
    assert_eq!(read.gross_pnl.checked_sub(read.fees), Some(read.net_pnl), "net is gross after fees");
    let count = |n: u64| Dec::from_int(i64::try_from(n).unwrap());
    assert_eq!(read.win_rate, share(count(read.wins), count(read.trades), 10));
    assert_eq!(read.profitable_trader_share, share(count(read.profitable_traders), count(read.traders), 10));
    assert_eq!(read.avg_trade_pnl, share(read.net_pnl, count(read.trades), 2));
    assert_eq!(read.avg_trader_pnl, share(read.net_pnl, count(read.traders), 2));
    assert_eq!(read.avg_fills_per_trade, share(count(read.fills), count(read.trades), 2));
    assert_eq!(read.funding_pnl, Dec::ZERO, "outcomes pay no funding");
    let held = read.hold_time.unwrap();
    assert!(held.p10_seconds < held.p50_seconds && held.p50_seconds < held.p95_seconds);
    assert_eq!(held.mean_seconds, dec("4852.51"));
}

#[test]
fn an_empty_profitability_window_has_no_hold_time() {
    let empty = PROFITABILITY.replace(
        r#""holdTime": {
    "meanSeconds": "4852.51",
    "p10Seconds": "0.94",
    "p50Seconds": "6.19",
    "p95Seconds": "19041.09"
  }"#,
        r#""holdTime": null"#,
    );
    let read: Profitability = serde_json::from_str(&empty).unwrap();
    assert_eq!(read.hold_time, None);
}

#[test]
fn each_request_names_its_route_and_its_bounds() {
    let named = Some(builder());
    let path = |call: Call| call.url().trim_start_matches("https://api.hydromancer.xyz").to_owned();
    assert_eq!(path(stats(named)), format!("/dashboard/outcome-builder-stats?builder={BUILDER}"));
    assert_eq!(path(stats(None)), "/dashboard/outcome-builder-stats?builder=null");
    assert_eq!(path(categories(named)), format!("/dashboard/outcome-builder-category-stats?builder={BUILDER}"));
    assert_eq!(path(hourly(named, 72)), format!("/dashboard/outcome-builder-hourly?builder={BUILDER}&hours=72"));
    assert_eq!(path(hourly(named, 1_000)), format!("/dashboard/outcome-builder-hourly?builder={BUILDER}&hours=168"));
    assert_eq!(path(hourly(named, 0)), format!("/dashboard/outcome-builder-hourly?builder={BUILDER}&hours=1"));
    assert_eq!(
        path(volume_daily("2026-01-01", None, 500)),
        "/dashboard/outcome-builder-volume-daily?startDate=2026-01-01&limitBuilders=100&format=packed"
    );
    assert_eq!(
        path(volume_daily("2026-06-01", Some("2026-06-03"), 2)),
        "/dashboard/outcome-builder-volume-daily?startDate=2026-06-01&endDate=2026-06-03&limitBuilders=2&format=packed"
    );
    assert_eq!(
        path(volume_daily("2026-06-01&builder=x", None, 2)),
        "/dashboard/outcome-builder-volume-daily?startDate=2026-06-01%26builder%3Dx&limitBuilders=2&format=packed",
        "a value cannot add a parameter"
    );
    assert_eq!(
        path(profitability(named, Span::Month)),
        format!("/dashboard/outcome-builder-profitability?builder={BUILDER}&window=30d")
    );
    let spans: Vec<&str> =
        [Span::Today, Span::Day, Span::Week, Span::Month, Span::Quarter, Span::All].map(Span::as_str).to_vec();
    assert_eq!(spans, ["today", "1d", "7d", "30d", "90d", "all"]);
}
