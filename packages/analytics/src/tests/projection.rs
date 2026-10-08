//! hip4-tui's `tui/src/tests/projection.rs`, the model's tests without the page, with instants
//! for its chrono times; the weekly names and the trendline's least squares worked out by hand.

use super::*;

fn at(text: &str) -> At {
    At::parse(text).unwrap()
}

fn plain() -> Projection {
    Projection {
        horizon: 1,
        users: 1,
        growth: 0,
        spend: 100_000_000,
        paying: 10_000,
        base: 1_000,
        scale: 100,
        deployer_reach: 10_000,
        builder_fee: 0,
        builder_reach: 10_000,
        referral_share: 1_000,
        referral_reach: 10_000,
        ..Projection::default()
    }
}

#[test]
fn a_trader_pays_the_base_times_the_scale_plus_what_the_protocol_keeps() {
    let rates = |scale: u32| {
        let projection = Projection { base: 100, scale, ..plain() };
        (projection.user_rate(), projection.deployer_rate())
    };
    assert_eq!(rates(100), (200, 100), "a scale of 1 is 2x, deployer 1x");
    assert_eq!(rates(300), (600, 300), "3 is 6x, deployer 3x");
    assert_eq!(rates(1_000), (2_000, 1_000), "10 is 20x, deployer 10x");
    assert_eq!(rates(50), (150, 50), "0.5 is 1.5x, deployer 0.5x");
    assert_eq!(rates(25), (125, 25), "0.25 is 1.25x, deployer 0.25x");
    assert_eq!(rates(0), (100, 0), "0 leaves the base and pays nothing");
}

#[test]
fn the_default_is_what_the_venue_actually_charged() {
    let projection = Projection::default();
    assert_eq!(projection.user_rate(), 1_344);
    assert_eq!(projection.deployer_rate(), 672, "half of it is the scale");
}

#[test]
fn only_the_volume_that_pays_a_fee_is_charged_one() {
    let projection = Projection { paying: 5_000, ..plain() };
    let one = projection.periods(at("2026-09-15T00:00:00Z"))[0];
    assert_eq!(one.volume, 100_000_000, "the whole million is traded");
    assert_eq!(one.paying, 50_000_000, "half of it closes");
    assert_eq!(one.charged, 100_000, "20 bp of what pays");
    assert_eq!(projection.charged_on(100_000_000), 100_000);
}

#[test]
fn a_fee_is_a_rate_on_what_pays_and_a_cut_of_what_that_comes_to() {
    let periods = plain().periods(at("2026-09-15T00:00:00Z"));
    let one = periods[0];
    assert_eq!(one.volume, 100_000_000, "one trader at a million");
    assert_eq!(one.paying, one.volume, "all of it closing, in this one");
    assert_eq!(one.charged, 200_000);
    assert_eq!(one.deployer, 100_000, "the scale half of it");
    assert_eq!(one.referral, 20_000, "a tenth of what they paid");
    assert_eq!(one.builder, 0, "the builder fee is off");
    assert_eq!(one.revenue, 120_000);
    assert_eq!(one.cumulative, one.revenue);
}

#[test]
fn a_builder_is_paid_on_buys_as_well_so_on_the_whole_volume() {
    let projection = Projection { paying: 5_000, builder_fee: 100, ..plain() };
    let one = projection.periods(at("2026-09-15T00:00:00Z"))[0];
    assert_eq!(one.builder, 10_000, "one basis point of the million");
}

#[test]
fn a_scale_is_set_per_market_so_only_your_own_pay_it_to_you() {
    let projection = Projection { deployer_reach: 1_000, ..plain() };
    let one = projection.periods(at("2026-09-15T00:00:00Z"))[0];
    assert_eq!(one.deployer, 10_000, "a tenth of the volume is yours");
    assert_eq!(one.charged, 200_000, "what traders pay is unchanged");
}

#[test]
fn one_rate_answers_on_volume_and_says_nothing_of_the_split() {
    let projection = Projection { mode: Mode::Total, blended: 500, ..plain() };
    let one = projection.periods(at("2026-09-15T00:00:00Z"))[0];
    assert_eq!(one.revenue, 50_000);
    assert_eq!((one.deployer, one.builder, one.referral), (0, 0, 0));
}

#[test]
fn growth_compounds_without_rounding_to_whole_people() {
    let projection = Projection { horizon: 3, users: 100, growth: 150, ..plain() };
    let periods = projection.periods(at("2026-09-15T00:00:00Z"));
    assert_eq!(periods[0].users, 100 * PER_USER);
    assert_eq!(periods[1].users, 101 * PER_USER + PER_USER / 2);
    assert_eq!(periods[2].users / PER_USER, 103);
}

#[test]
fn a_decline_is_a_projection_too_and_never_goes_below_nobody() {
    let projection = Projection { horizon: 4, users: 1_000, growth: -10_000, ..plain() };
    let periods = projection.periods(at("2026-09-15T00:00:00Z"));
    assert_eq!(periods[0].users / PER_USER, 1_000);
    assert_eq!(periods[1].users, 0, "everybody left");
    assert_eq!(periods[3].revenue, 0);
}

#[test]
fn the_horizon_names_its_own_periods() {
    let mut projection = Projection { horizon: 3, step: Step::Month, ..plain() };
    let names = |projection: &Projection| -> Vec<String> {
        projection.periods(at("2026-11-15T00:00:00Z")).iter().map(|one| projection.step.name(one.at)).collect()
    };
    assert_eq!(names(&projection), ["Nov 2026", "Dec 2026", "Jan 2027"]);
    projection.step = Step::Quarter;
    assert_eq!(names(&projection), ["Q4 2026", "Q1 2027", "Q2 2027"]);
    projection.step = Step::Week;
    assert_eq!(names(&projection), ["15 Nov", "22 Nov", "29 Nov"]);
}

#[test]
fn a_start_moves_the_whole_horizon_and_nothing_else() {
    let projection = Projection { horizon: 2, start: 2, ..plain() };
    let periods = projection.periods(at("2026-11-15T00:00:00Z"));
    assert_eq!(projection.step.name(periods[0].at), "Jan 2027");
    assert_eq!(periods[0].revenue, plain().periods(at("2026-11-15T00:00:00Z"))[0].revenue);
    let back = Projection { start: -1, ..plain() };
    assert_eq!(back.step.name(back.periods(at("2026-11-15T00:00:00Z"))[0].at), "Oct 2026");
}

#[test]
fn totals_are_the_horizon_and_a_year_at_the_last_period_s_rate() {
    let projection = Projection { horizon: 3, ..plain() };
    let periods = projection.periods(at("2026-09-15T00:00:00Z"));
    let totals = Projection::totals(&periods);
    assert_eq!(totals.volume, 300_000_000, "three periods of a million");
    assert_eq!(totals.revenue, 360_000);
    assert_eq!(totals.deployer, 300_000, "the three deployer cells");
    assert_eq!(totals.revenue, periods[2].cumulative, "the total and the last running total are the same figure");
    assert_eq!(totals.users, periods[2].users, "a count is where it ends");
    let a_year = Projection::a_year(&periods, projection.step);
    assert_eq!(a_year.revenue, 120_000 * 12, "twelve of the last month");
    assert_eq!(Projection::a_year(&[], Step::Week), Totals::default());
}

#[test]
fn the_horizon_is_held_to_five_years_of_months() {
    let projection = Projection { horizon: 500, ..plain() };
    assert_eq!(projection.periods(at("2026-09-15T00:00:00Z")).len(), MAX_PERIODS as usize);
}

#[test]
fn each_source_is_its_own_series() {
    let projection = Projection { horizon: 2, builder_fee: 100, ..plain() };
    let periods = projection.periods(at("2026-09-15T00:00:00Z"));
    assert_eq!(Projection::series(Source::Deployer, &periods), [100_000, 100_000]);
    assert_eq!(Projection::series(Source::Builder, &periods), [10_000, 10_000]);
    assert_eq!(Projection::series(Source::Referral, &periods), [20_000, 20_000]);
    assert_eq!(Projection::series(Source::All, &periods), [130_000, 130_000]);
}

#[test]
fn a_trendline_is_the_least_squares_line_through_the_series_never_below_nothing() {
    assert_eq!(trend(&[10, 20, 30]), [10, 20, 30], "a line is its own fit");
    assert_eq!(trend(&[0, 0, 30]), [0, 10, 25], "-5 at the start, held at nothing");
    assert_eq!(trend(&[7]), [7], "one point has no slope");
    assert!(trend(&[]).is_empty());
}

#[test]
fn a_typed_figure_is_read_in_the_row_s_own_unit() {
    let mut projection = Projection::default();
    assert!(Row::Growth.take(&mut projection, "7.5"));
    assert_eq!(projection.growth, 750, "seven and a half per cent");
    assert!(Row::Base.take(&mut projection, "6.72"));
    assert_eq!(projection.base, 672);
    assert!(Row::Scale.take(&mut projection, "2.5"));
    assert_eq!(projection.scale, 250, "two and a half times the base");
    assert!(Row::Spend.take(&mut projection, "1250.50"));
    assert_eq!(projection.spend, 125_050);
    assert!(Row::Users.take(&mut projection, "9000"));
    assert_eq!(projection.users, 9_000);
    assert!(!Row::Step.take(&mut projection, "3"), "a choice takes none");
    assert!(!Row::Users.take(&mut projection, "many"));
}

#[test]
fn a_step_stays_inside_what_the_figure_can_mean() {
    let mut projection = Projection { scale: 0, ..Projection::default() };
    Row::Scale.bump(&mut projection, -10);
    assert_eq!(projection.scale, 0, "no scale below none");
    for _ in 0..500 {
        Row::Scale.bump(&mut projection, 10);
    }
    assert_eq!(projection.scale, MAX_SCALE, "and none past the ten the venue allows");
    Row::Step.bump(&mut projection, 1);
    assert_eq!(projection.step, Step::Quarter, "month, then quarter");
    Row::Step.bump(&mut projection, -1);
    assert_eq!(projection.step, Step::Month, "and back");
    Row::Mode.bump(&mut projection, 1);
    assert_eq!(projection.mode, Mode::Total);
}

#[test]
fn one_rate_mode_dims_the_rows_it_does_not_read() {
    let mut projection = Projection::default();
    assert!(Row::Scale.live(&projection));
    assert!(!Row::Blended.live(&projection));
    projection.mode = Mode::Total;
    assert!(!Row::Scale.live(&projection));
    assert!(!Row::Base.live(&projection));
    assert!(Row::Blended.live(&projection));
    assert!(Row::Users.live(&projection), "who trades still counts");
}

#[test]
fn every_section_of_the_assumptions_is_one_word() {
    let mut groups: Vec<&str> = Row::ALL.into_iter().map(Row::group).collect();
    groups.dedup();
    for group in &groups {
        assert!(!group.contains(' '), "`{group}` is a sentence where a word would do");
    }
    assert_eq!(groups, vec!["when", "traders", "fees"]);
}
