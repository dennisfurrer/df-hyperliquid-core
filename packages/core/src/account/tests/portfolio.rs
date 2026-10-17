use super::*;
use crate::account::fixtures::{PORTFOLIO_UNIFIED, Recorded, UNIFIED, address, block_on, dec, read};

#[test]
fn every_span_reads_and_the_day_says_what_the_account_is_worth_now() {
    let portfolio: Portfolio = read(PORTFOLIO_UNIFIED);
    let spans: Vec<&Span> = portfolio.spans.iter().map(|(span, _)| span).collect();
    assert_eq!(
        spans,
        [
            &Span::Day,
            &Span::Week,
            &Span::Month,
            &Span::AllTime,
            &Span::PerpDay,
            &Span::PerpWeek,
            &Span::PerpMonth,
            &Span::PerpAllTime
        ]
    );
    assert_eq!(portfolio.value(), Some(dec("1279262.5624760001")));
    assert_eq!(portfolio.today(), Some(dec("-78492.656033")), "the day's profit, a running sum from zero");
    let day = portfolio.span(&Span::Day).unwrap();
    assert_eq!(day.account_value_history.len(), 13);
    assert_eq!(day.account_value_history[12], (1_791_480_974_304, dec("1279262.5624760001")));
    assert_eq!(day.vlm, dec("18488907.4499999993"));
    let perps = portfolio.span(&Span::PerpDay).unwrap();
    assert_eq!(perps.account_value_history.last().map(|point| point.1), Some(dec("549463.356168")), "perps alone");
}

#[test]
fn a_span_this_build_does_not_know_is_kept_by_its_name() {
    let portfolio: Portfolio =
        read(r#"[["fortnight",{"accountValueHistory":[[1,"2.5"]],"pnlHistory":[],"vlm":"0.0"}]]"#);
    assert_eq!(portfolio.spans[0].0, Span::Other("fortnight".to_owned()));
    assert_eq!(portfolio.span(&Span::Other("fortnight".to_owned())).map(|history| history.vlm), Some(Dec::ZERO));
    assert_eq!((portfolio.value(), portfolio.today()), (None, None), "no day");
    assert_eq!(Span::parse("perpAllTime"), Span::PerpAllTime);
    assert_eq!(Span::PerpAllTime.wire(), "perpAllTime");
}

/// Accounts are summed at the times of the longest series, each account taken at or before them.
#[test]
fn several_accounts_sum_at_the_longest_series_times() {
    let one = [(1, dec("10")), (3, dec("30")), (5, dec("50"))];
    let two = [(2, dec("1")), (4, dec("2"))];
    assert_eq!(summed(&[&one, &two]), [(1, dec("10")), (3, dec("31")), (5, dec("52"))]);
    assert_eq!(summed(&[]), []);
}

#[test]
fn the_portfolio_is_read_at_the_address() {
    let body = format!(r#"{{"type":"portfolio","user":"{UNIFIED}"}}"#);
    assert_eq!(portfolio_request(address(UNIFIED)), body);
    let transport = Recorded::new(&[(&body, PORTFOLIO_UNIFIED)]);
    assert_eq!(block_on(portfolio(&transport, address(UNIFIED))).unwrap().spans.len(), 8);
}
