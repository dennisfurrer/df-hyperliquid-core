//! hip4-tui's `engine/src/tests/state.rs` on the venue rollup, its `crowded` listing written out
//! as the venue would send it; and `outcomeMeta` as mainnet answered it on 2026-09-26 (hip4-tui's
//! `tui/src/app/tests/fixtures`: 233 outcomes from three HIP-4 deployers and the venue's own, 98
//! of them legs of 20 questions) beside `outcomeTemplates` of 2026-09-28. The counts were taken
//! from the same files with Python's `json`.

use hl_core::Address;
use hl_hip4::{Catalog, Family, OutcomeMeta, OutcomeTemplate, RateMove, Remaining, Templates};

use super::*;

const LIVE: &str = include_str!("../../fixtures/outcome_meta_mainnet_20260926.json");
const TEMPLATES: &str = include_str!("../../fixtures/outcome_templates_mainnet.json");

/// 2026-09-26 20:00 UTC, when the live listing above was read.
const READ_AT: u64 = 1_790_452_800_000;

/// 2026-09-30 12:00 UTC: the crowded venue's question has passed its deadline and one of its
/// markets has expired, and none of them is settled.
const LATE_SEPTEMBER: u64 = 1_790_769_600_000;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn live() -> OutcomeMeta {
    serde_json::from_str(LIVE).unwrap()
}

fn templates() -> Vec<OutcomeTemplate> {
    serde_json::from_str::<Templates>(TEMPLATES).unwrap().0
}

fn row(outcome: u32, name: &str, description: &str, venue: Option<&str>) -> String {
    let venue = venue.map_or_else(String::new, |venue| format!(r#","venue":"{venue}""#));
    format!(
        r#"{{"outcome":{outcome},"name":"{name}","description":"{description}","sideSpecs":[{{"name":"Yes"}},{{"name":"No"}}],"quoteToken":"USDC"{venue}}}"#
    )
}

/// Two open markets on `out`, one expired and unsettled, and a question past its deadline with
/// one named leg and its fallback; beside them a market on `txyz` and one that names no venue.
fn crowded() -> OutcomeMeta {
    let rows = [
        row(1209, "template:binaryPrice4", "perp:BTC|threshold:100000|time:20261001-0000", Some("out")),
        row(1210, "template:binaryPrice4", "perp:BTC|threshold:110000|time:20261001-0000", Some("out")),
        row(1100, "template:binaryPrice4", "perp:BTC|threshold:90000|time:20260901-0000", Some("out")),
        row(1472, "template fallback", "other", Some("out")),
        row(1473, "template:sportsTournamentParticipant", "participant:Arsenal", Some("out")),
        row(2494, "template:binaryPrice4", "perp:ETH|threshold:5000|time:20261001-0000", Some("txyz")),
        row(3719, "template:binaryPrice4", "perp:HYPE|threshold:50|time:20261001-0000", None),
    ];
    serde_json::from_str(&format!(
        r#"{{"outcomes":[{}],
        "questions":[{{"question":198,"name":"template:sportsTournamentWinner",
            "description":"competition:English Premier League|resolutionDeadline:20260915-1200|season:2026/2027|sport:football",
            "fallbackOutcome":1472,"namedOutcomes":[1473],"settledNamedOutcomes":[]}}],
        "deployers":[{{"deployer":"0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e","venue":"out","subDeployers":[]}},
                     {{"deployer":"0x00000000000000000000000000000000000000aa","venue":"txyz","subDeployers":[]}}]}}"#,
        rows.join(",")
    ))
    .unwrap()
}

fn find<'a>(venues: &'a [Venue], name: Option<&str>) -> &'a Venue {
    venues.iter().find(|venue| venue.venue.as_deref() == name).unwrap()
}

#[test]
fn a_venue_s_live_count_is_every_row_it_holds_as_its_cap_counts_them() {
    let meta = crowded();
    let venues = venues(&meta, &Catalog::build(&meta, LATE_SEPTEMBER));
    let ours = find(&venues, Some("out"));
    assert_eq!(ours.open, 2, "two markets are still before their deadline");
    assert_eq!(ours.live, 5, "the expired market, the question's leg and its fallback are still on the venue");
    assert_eq!(ours.markets, 5);
    assert_eq!(ours.questions, 1);
    assert_eq!(find(&venues, Some("txyz")).live, 1);
    assert_eq!(find(&venues, None).live, 0, "a row that names no venue counts toward no venue's cap");
}

#[test]
fn the_kinds_shown_narrow_what_is_listed_never_what_the_cap_counts() {
    let meta = crowded();
    let mut catalog = Catalog::build(&meta, LATE_SEPTEMBER);
    catalog.retain_venue("out");
    catalog.retain_kinds(&[Family::BinaryPrice]);
    let venues = venues(&meta, &catalog);
    assert_eq!(venues.len(), 1);
    assert_eq!((venues[0].markets, venues[0].live), (3, 5));
}

#[test]
fn the_first_market_a_venue_listed_is_its_lowest_outcome_id() {
    let meta = crowded();
    let catalog = Catalog::build(&meta, LATE_SEPTEMBER);
    let listed = venues(&meta, &catalog);
    let first = find(&listed, Some("out")).first.as_ref().unwrap();
    assert_eq!(first.outcome, 1100, "the lowest id of ours, not the first row that named us");
    assert_eq!(
        Some(first.title.as_str()),
        catalog.markets.iter().find(|one| one.outcome == 1100).map(|one| one.title.as_str())
    );
    assert_eq!(find(&listed, Some("txyz")).first.as_ref().map(|one| one.outcome), Some(2494));
    assert_eq!(find(&listed, None).first.as_ref().map(|one| one.outcome), Some(3719));
    assert!(venues(&OutcomeMeta::default(), &Catalog::default()).is_empty(), "nothing listed, no venue");
}

#[test]
fn the_live_venues_are_ranked_by_what_they_list_with_their_deployers() {
    let meta = live();
    let venues = venues(&meta, &Catalog::build(&meta, READ_AT));
    let rows: Vec<(Option<&str>, usize, usize, usize)> =
        venues.iter().map(|venue| (venue.venue.as_deref(), venue.markets, venue.questions, venue.live)).collect();
    assert_eq!(
        rows,
        [(Some("out"), 100, 16, 100), (Some("txyz"), 66, 3, 66), (Some("skew"), 59, 0, 59), (None, 8, 1, 0)],
        "the venue's own daily markets last, with no deployer and no cap"
    );
    let deployers: Vec<Option<Address>> = venues.iter().map(|venue| venue.deployer).collect();
    assert_eq!(
        deployers,
        [
            Some(address("0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e")),
            Some(address("0x7aca09667816a4817b8bc697fb239ad42ff9f553")),
            Some(address("0x08e9c89f46dccee91bdb85c6532eb93a4c335efe")),
            None
        ]
    );
    let firsts: Vec<Option<u32>> = venues.iter().map(|venue| venue.first.as_ref().map(|one| one.outcome)).collect();
    assert_eq!(firsts, [Some(1209), Some(2494), Some(4724), Some(5755)]);
    let keys: Vec<Vec<usize>> =
        venues.iter().map(|venue| venue.subs.iter().map(|sub| sub.actions.len()).collect()).collect();
    assert_eq!(keys, [vec![5, 5, 5], vec![5, 5], vec![5], vec![]], "each key with the five operations it may send");
    assert!(venues.iter().all(|venue| venue.open <= venue.markets));
}

#[test]
fn a_venue_at_a_hundred_live_rows_has_reached_the_cap_the_venue_counts() {
    let meta = live();
    let venues = venues(&meta, &Catalog::build(&meta, READ_AT));
    let out = find(&venues, Some("out"));
    let limits = out.limits(Remaining { daily: 452, active: 0 });
    assert_eq!((limits.active_used, limits.active_cap(), limits.daily_remaining), (100, 100, 452));
    let txyz = find(&venues, Some("txyz")).limits(Remaining { daily: 10, active: 34 });
    assert_eq!(txyz.active_cap(), 100, "live plus remaining, never a constant");
}

#[test]
fn a_venue_s_templates_are_counted_from_the_names_of_what_it_listed() {
    let meta = live();
    let published = templates();
    let used = templates_of(&meta, Some("out"), &published);
    let counts: Vec<(&str, usize, usize, bool)> =
        used.iter().map(|one| (one.id.as_str(), one.outcomes, one.questions, one.template.is_some())).collect();
    assert_eq!(
        counts,
        [
            ("sportsContestParticipant2", 24, 0, false),
            ("sportsTournamentParticipant", 18, 0, true),
            ("sportsContestWinner", 16, 0, true),
            ("sportsContestDraw2", 12, 0, false),
            ("sportsContestResult", 0, 12, false),
            ("priceTouch", 7, 0, true),
            ("binaryPrice", 4, 0, true),
            ("sportsTournamentWinner", 0, 3, true),
            ("policyRateDecision", 0, 1, false),
            ("policyRateDecrease", 1, 0, false),
            ("policyRateIncrease", 1, 0, false),
            ("policyRateNoChange", 1, 0, false),
        ],
        "a template the venue no longer publishes is still counted, unlinked"
    );
    assert_eq!(used[0].family, Family::ContestParticipant);
    assert_eq!(used[11].family, Family::PolicyRate(RateMove::NoChange));
    assert_eq!(used[4].family, Family::ContestResult);

    let skew = templates_of(&meta, Some("skew"), &published);
    assert_eq!(skew.len(), 1);
    assert_eq!((skew[0].id.as_str(), skew[0].outcomes), ("binaryPrice", 59));
    assert!(templates_of(&meta, None, &published).is_empty(), "the venue's own markets come from no template");
}
