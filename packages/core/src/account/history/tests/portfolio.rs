//! `portfolio` as the venue writes it, the unified account's, and several accounts' series summed.
//! The read runs in the mocked journeys.

use super::*;
use crate::account::fixtures::{PORTFOLIO_UNIFIED, dec, read};

#[test]
fn every_span_reads_by_name_and_the_day_says_what_the_account_is_worth_now() {
    let portfolio: Portfolio = read(PORTFOLIO_UNIFIED);
    let spans: Vec<&str> = portfolio.spans.iter().map(|(span, _)| span.wire()).collect();
    assert_eq!(spans, ["day", "week", "month", "allTime", "perpDay", "perpWeek", "perpMonth", "perpAllTime"]);
    for (span, _) in &portfolio.spans {
        assert_eq!(&Span::parse(&span.to_string()), span, "{span} is no Other");
    }
    assert_eq!(portfolio.value(), Some(dec("1279262.5624760001")));
    assert_eq!(portfolio.today(), Some(dec("-78492.656033")), "the day's profit, a running sum from zero");
    let day = portfolio.span(&Span::Day).unwrap();
    assert_eq!(day.account_value_history.len(), 13);
    assert_eq!(day.account_value_history[12], (1_791_480_974_304, dec("1279262.5624760001")));
    assert_eq!(day.vlm, dec("18488907.4499999993"));
    let perps = portfolio.span(&Span::PerpDay).unwrap();
    assert_eq!(perps.account_value_history.last().map(|point| point.1), Some(dec("549463.356168")), "perps alone");

    let unknown: Portfolio = read(r#"[["fortnight",{"accountValueHistory":[[1,"2.5"]],"pnlHistory":[],"vlm":"0.0"}]]"#);
    let fortnight = Span::Other("fortnight".to_owned());
    assert_eq!((&unknown.spans[0].0, fortnight.wire()), (&fortnight, "fortnight"));
    assert_eq!(unknown.span(&fortnight).map(|history| history.vlm), Some(Dec::ZERO));
    assert_eq!((unknown.value(), unknown.today()), (None, None), "no day");
}

/// Accounts are summed at the times of the longest series, each account taken at or before them.
#[test]
fn several_accounts_sum_at_the_longest_series_times() {
    let one = [(1, dec("10")), (3, dec("30")), (5, dec("50"))];
    let two = [(2, dec("1")), (4, dec("2"))];
    assert_eq!(summed(&[&one, &two]), [(1, dec("10")), (3, dec("31")), (5, dec("52"))]);
    assert_eq!(summed(&[]), []);
}
