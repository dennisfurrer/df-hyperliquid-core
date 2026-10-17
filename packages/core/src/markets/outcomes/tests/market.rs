//! `outcomeMeta` as the venue answered it, trimmed: mainnet on 2026-09-28 with its templates, and
//! testnet on 2026-10-08. The two networks number their outcomes apart; nothing below assumes an
//! id it did not read.

use super::*;
use crate::markets::outcomes::meta::{Outcome, OutcomeMeta, Question as RawQuestion, SideSpec};
use crate::markets::outcomes::template::{Pattern, Templates, patterns};

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_meta_mainnet.json"));
const MAINNET_TEMPLATES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_templates_mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip4/outcome_meta_testnet.json"));

/// 2026-09-10 00:00 UTC.
const NOW: u64 = 1_788_998_400_000;
/// 2026-09-28 19:36:40 UTC, just after the mainnet answer was recorded.
const MAINNET_NOW: u64 = 1_790_624_200_000;
/// 2026-10-08 00:00 UTC, the day the testnet answer was recorded.
const TESTNET_NOW: u64 = 1_791_417_600_000;

fn coin(text: &str) -> Coin {
    Coin::lookup(text).unwrap()
}

fn outcome(id: u32, name: &str, description: &str, venue: &str) -> Outcome {
    Outcome {
        outcome: id,
        name: name.to_owned(),
        description: description.to_owned(),
        side_specs: vec![
            SideSpec { name: "template:{shortNameA}".to_owned() },
            SideSpec { name: "template:{shortNameB}".to_owned() },
        ],
        quote_token: Some("USDC".to_owned()),
        venue: Some(venue.to_owned()),
        deployer_fee_scale: Some(Dec::ONE),
    }
}

fn question(id: u32, name: &str, description: &str, fallback: Option<u32>, named: Vec<u32>) -> RawQuestion {
    RawQuestion {
        question: id,
        name: name.to_owned(),
        description: description.to_owned(),
        fallback_outcome: fallback,
        named_outcomes: named,
        settled_named_outcomes: Vec::new(),
    }
}

fn meta(outcomes: Vec<Outcome>, questions: Vec<RawQuestion>) -> OutcomeMeta {
    OutcomeMeta { outcomes, questions, ..OutcomeMeta::default() }
}

fn one(outcome: u32, title: &str, venue: Option<&str>) -> Market {
    Market {
        market: Coin::new(outcome, OutcomeSide::Yes),
        outcome,
        kind: Family::BinaryPrice,
        title: title.to_owned(),
        template: String::new(),
        template_version: None,
        description: String::new(),
        status: Status::Open,
        expiry: None,
        venue: venue.map(str::to_owned),
        quote: None,
        fee_scale: None,
        question: None,
        question_title: None,
        sides: Sides {
            yes: Coin::new(outcome, OutcomeSide::Yes),
            no: Coin::new(outcome, OutcomeSide::No),
            yes_label: "Yes".to_owned(),
            no_label: "No".to_owned(),
        },
        details: Details::Prose { text: String::new() },
    }
}

#[test]
fn a_market_is_found_by_either_side_of_it() {
    let catalog =
        Catalog::of(vec![one(1210, "first", None), one(1211, "second", None), one(9999, "third", None)], vec![]);
    for outcome in [1210u32, 1211, 9999] {
        for side in [OutcomeSide::Yes, OutcomeSide::No] {
            let found = catalog.find(Coin::new(outcome, side)).map(|market| market.outcome);
            assert_eq!(found, Some(outcome), "{outcome} by its {side:?} coin");
        }
    }
    assert!(catalog.find(Coin::new(1, OutcomeSide::Yes)).is_none());
}

#[test]
fn narrowing_a_catalogue_takes_the_index_with_it() {
    let mut catalog = Catalog::of(
        vec![
            one(1210, "a market", Some("out")),
            one(1220, "a market", Some("txyz")),
            one(1230, "a market", Some("out")),
        ],
        vec![],
    );
    assert!(catalog.find(coin("#12300")).is_some());
    catalog.retain_venue("out");
    assert_eq!(catalog.markets.len(), 2);
    assert_eq!(catalog.find(coin("#12300")).map(|market| market.outcome), Some(1230), "the index moved with it");
    assert!(catalog.find(coin("#12200")).is_none());
}

#[test]
fn a_search_ignores_case() {
    assert!(contains("Elche CF vs Real Madrid", "real"));
    assert!(contains("Elche CF vs Real Madrid", "REAL"));
    assert!(contains("btc above", "BTC"));
    assert!(!contains("btc above", "eth"));
    assert!(contains("anything", ""));
    assert!(!contains("no", "nothing"));
}

#[test]
fn a_binary_price_market_reads_end_to_end() {
    let meta = meta(
        vec![outcome(
            1210,
            "template:binaryPrice4",
            "perp:BTC|priceDescription:BTC-USDC mark|seconds:300|threshold:100000|time:20261001-0000",
            "out",
        )],
        vec![],
    );
    let catalog = Catalog::build(&meta, NOW);
    let market = &catalog.markets[0];
    assert_eq!(market.kind, Family::BinaryPrice);
    assert_eq!(market.template_version, Some(4));
    assert_eq!((market.market.to_string(), market.sides.no.to_string()), ("#12100".to_owned(), "#12101".to_owned()));
    assert_eq!(market.status, Status::Open);
    assert_eq!(market.title, "BTC above 100000 at 2026-10-01 00:00Z");
    assert_eq!(market.fee_scale, Some(Dec::ONE), "read back as 1.0");
    assert!(matches!(market.details, Details::Price { window_seconds: Some(300), .. }));
    let after = Catalog::build(&meta, Stamp::parse("20261001-0000").unwrap().ms().unsigned_abs());
    assert_eq!(after.markets[0].status, Status::Expired, "from its own minute on");
}

#[test]
fn short_names_fill_the_side_labels() {
    let meta = meta(
        vec![outcome(
            1865,
            "template:sportsContestWinner",
            "competition:NFL|participantA:San Francisco 49ers|participantB:Los Angeles Rams|resolutionDeadline:20260911-0635|shortNameA:49ers|shortNameB:Rams|stage:Regular Season",
            "txyz",
        )],
        vec![],
    );
    let market = &Catalog::build(&meta, NOW).markets[0];
    assert_eq!((market.sides.yes_label.as_str(), market.sides.no_label.as_str()), ("49ers", "Rams"));
    assert_eq!(market.title, "San Francisco 49ers vs Los Angeles Rams (NFL Regular Season)");
}

#[test]
fn legs_inherit_the_deadline_and_title_of_their_question() {
    let mut raw = question(
        198,
        "template:sportsTournamentWinner",
        "competition:English Premier League|resolutionDeadline:20270605-1200|season:2026/2027|sport:football",
        Some(1472),
        vec![1473],
    );
    raw.settled_named_outcomes = vec![9999];
    let meta = meta(
        vec![
            outcome(1472, "template fallback", "other", "out"),
            outcome(1473, "template:sportsTournamentParticipant", "participant:Arsenal", "out"),
        ],
        vec![raw],
    );
    let catalog = Catalog::build(&meta, NOW);
    let asked = &catalog.questions[0];
    assert_eq!(asked.title, "English Premier League 2026/2027 winner");
    assert_eq!(asked.venue.as_deref(), Some("out"));
    assert_eq!(asked.legs, vec![coin("#14730")]);
    assert_eq!(asked.settled_legs, 1);
    let leg = catalog.find(coin("14730")).unwrap();
    assert_eq!(leg.title, "Arsenal");
    assert_eq!(leg.question, Some(198));
    assert_eq!(leg.expiry, asked.expiry, "a leg with no deadline takes the question's");
    assert_eq!(leg.headline(), "English Premier League 2026/2027 winner / Arsenal");
    assert!(leg.matches("arsenal") && leg.matches("premier") && leg.matches("#1473"));
    let fallback = catalog.find(coin("#14720")).unwrap();
    assert_eq!((fallback.kind, fallback.title.as_str()), (Family::Fallback, "none of the listed outcomes"));
    assert_eq!(catalog.place_of(198), Some(0));
}

#[test]
fn bucket_legs_take_their_range_from_the_question() {
    let mut leg = outcome(2251, "Recurring Named Outcome", "index:1", "out");
    leg.venue = None;
    let meta = meta(
        vec![leg],
        vec![question(
            239,
            "Recurring",
            "class:priceBucket|underlying:BTC|expiry:20260911-0600|priceThresholds:76929,80069|period:1d",
            None,
            vec![2251],
        )],
    );
    let catalog = Catalog::build(&meta, NOW);
    assert_eq!(catalog.questions[0].title, "BTC price range at 2026-09-11 06:00Z");
    assert_eq!(catalog.markets[0].title, "BTC 76929 to 80069");
    assert_eq!(catalog.markets[0].expiry, catalog.questions[0].expiry);
}

#[test]
fn chain_numbers_reach_a_title_exactly_as_written() {
    let meta =
        meta(vec![outcome(1211, "template:priceTouch", "perp:BTC|target:95000.00|time:20261001-0000", "out")], vec![]);
    let market = &Catalog::build(&meta, NOW).markets[0];
    assert_eq!(market.title, "BTC touches 95000.00 by 2026-10-01 00:00Z");
    assert!(matches!(&market.details, Details::Price { target: Some(target), .. } if target == "95000.00"));
}

#[test]
fn a_catalog_filters_by_venue_and_by_kind() {
    let meta = meta(
        vec![
            outcome(1210, "template:binaryPrice", "perp:BTC|threshold:1", "out"),
            outcome(1211, "template:priceTouch", "perp:BTC|target:1", "txyz"),
        ],
        vec![],
    );
    let mut catalog = Catalog::build(&meta, NOW);
    catalog.retain_venue("out");
    assert_eq!(catalog.markets.len(), 1);
    assert_eq!(catalog.markets[0].kind, Family::BinaryPrice);
    catalog.retain_kinds(&[Family::PriceTouch]);
    assert!(catalog.markets.is_empty());
}

/// The venue's patterns for templates mainnet listed on 2026-10-01 that had no family here, as
/// `outcomeTemplates` gave them.
fn new_patterns() -> Patterns {
    let one = |id: &str, name: &str, sides: &[&str], keywords: &[(&str, &str)]| Pattern {
        id: id.to_owned(),
        name: name.to_owned(),
        sides: sides.iter().map(|side| (*side).to_owned()).collect(),
        keywords: keywords.iter().map(|(name, kind)| ((*name).to_owned(), (*kind).to_owned())).collect(),
    };
    [
        one(
            "companyIpoConfirmed",
            "{company} IPO confirmed by {dateTime}",
            &["Yes", "No"],
            &[("company", "string"), ("dateTime", "dateTime")],
        ),
        one(
            "sportsSpread",
            "{participantA} v {participantB}: {shortNameA} {spread}",
            &["Yes", "No"],
            &[
                ("countedPlay", "string"),
                ("measure", "string"),
                ("officialSource", "string"),
                ("participantA", "string"),
                ("participantB", "string"),
                ("resolutionDeadline", "dateTime"),
                ("scheduledStart", "dateTime"),
                ("shortNameA", "shortString"),
                ("sport", "string"),
                ("spread", "string"),
            ],
        ),
        one(
            "awardWinner",
            "{period} {award} winner",
            &[],
            &[("award", "string"), ("period", "string"), ("resolutionDeadline", "dateTime")],
        ),
        one("awardCandidate", "{candidate}", &[], &[("candidate", "string")]),
    ]
    .into_iter()
    .map(|pattern| (pattern.id.clone(), pattern))
    .collect()
}

#[test]
fn a_template_with_no_family_reads_from_the_venue_s_pattern() {
    let meta = meta(
        vec![
            outcome(2597, "template:companyIpoConfirmed", "company:Stripe|dateTime:20261031-2359", "txyz"),
            outcome(
                7007,
                "template:sportsSpread",
                "countedPlay:regulation time and any overtime|measure:points|officialSource:ESPN|participantA:Pittsburgh Steelers|participantB:Cleveland Browns|resolutionDeadline:20261002-0615|scheduledStart:20261002-0015|shortNameA:Steelers|sport:football|spread:-2.5",
                "txyz",
            ),
        ],
        vec![],
    );
    let catalog = Catalog::build_with(&meta, &new_patterns(), NOW);
    let ipo = catalog.find(coin("#25970")).unwrap();
    assert_eq!(ipo.kind, Family::Template);
    assert_eq!(ipo.title, "Stripe IPO confirmed by 2026-10-31 23:59Z");
    assert_eq!(ipo.expiry.map(|at| at.to_string()).as_deref(), Some("2026-10-31 23:59Z"));
    let Details::Template { template, fields } = &ipo.details else {
        panic!("read as {:?}", ipo.details);
    };
    assert_eq!(template, "companyIpoConfirmed");
    assert_eq!(
        fields,
        &vec![("company".to_owned(), "Stripe".to_owned()), ("dateTime".to_owned(), "2026-10-31 23:59Z".to_owned())]
    );
    let spread = catalog.find(coin("#70070")).unwrap();
    let Details::Template { fields, .. } = &spread.details else {
        panic!("read as {:?}", spread.details);
    };
    let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names[..4],
        ["participantA", "participantB", "shortNameA", "spread"],
        "what the title is written from leads"
    );
    assert_eq!(spread.title, "Pittsburgh Steelers v Cleveland Browns: Steelers -2.5");
    assert_eq!(
        spread.expiry.map(|at| at.to_string()).as_deref(),
        Some("2026-10-02 06:15Z"),
        "the resolution deadline, not the scheduled start"
    );
}

#[test]
fn before_the_patterns_land_a_template_still_reads_its_values() {
    let meta = meta(
        vec![outcome(2597, "template:companyIpoConfirmed", "company:Stripe|dateTime:20261031-2359", "txyz")],
        vec![],
    );
    let market = &Catalog::build(&meta, NOW).markets[0];
    assert_eq!(market.title, "Stripe, 2026-10-31 23:59Z");
    assert_eq!(market.expiry.map(|at| at.to_string()).as_deref(), Some("2026-10-31 23:59Z"));
}

#[test]
fn a_question_on_a_new_template_names_its_legs_and_lends_them_its_deadline() {
    let meta = meta(
        vec![
            outcome(20085, "template:awardCandidate", "candidate:Black Bag", "ao"),
            outcome(20087, "template:awardCandidate", "", "ao"),
        ],
        vec![question(
            1376,
            "template:awardWinner",
            "award:Booker Prize|period:2026|resolutionDeadline:20261111-2359",
            None,
            vec![20085, 20087],
        )],
    );
    let catalog = Catalog::build_with(&meta, &new_patterns(), NOW);
    let asked = &catalog.questions[0];
    assert_eq!(asked.title, "2026 Booker Prize winner");
    let leg = catalog.find(coin("#200850")).unwrap();
    assert_eq!(leg.title, "Black Bag");
    assert_eq!(leg.expiry, asked.expiry);
    assert_eq!(catalog.find(coin("#200870")).unwrap().title, "?", "a keyword the outcome leaves out reads as ?");
}

#[test]
fn mainnet_s_catalogue_reads_every_shape_it_holds() {
    let meta: OutcomeMeta = serde_json::from_str(MAINNET).unwrap();
    let templates = serde_json::from_str::<Templates>(MAINNET_TEMPLATES).unwrap().0;
    let catalog = Catalog::build_with(&meta, &patterns(&templates), MAINNET_NOW);
    assert_eq!((catalog.markets.len(), catalog.questions.len()), (15, 2));

    let touch = catalog.find(coin("#12090")).unwrap();
    assert_eq!(touch.title, "HYPE touches 100 by 2026-10-01 00:00Z");
    assert_eq!((touch.kind, touch.venue.as_deref()), (Family::PriceTouch, Some("out")));

    let game = catalog.find(coin("#46670")).unwrap();
    assert_eq!(game.title, "Philadelphia Eagles vs Chicago Bears (NFL Regular Season)");
    assert_eq!((game.sides.yes_label.as_str(), game.sides.no_label.as_str()), ("Eagles", "Bears"));
    assert_eq!(game.expiry.map(|at| at.to_string()).as_deref(), Some("2026-09-29 06:15Z"));
    assert_eq!(game.status, Status::Open);

    let daily = catalog.find(coin("#62970")).unwrap();
    assert_eq!((daily.kind, daily.venue.as_deref()), (Family::Recurring, None), "the venue's own, no deployer");
    assert_eq!(daily.title, "BTC above 83142 at 2026-09-29 06:00Z");
    assert!(matches!(&daily.details, Details::Recurring { period: Some(period), .. } if period == "1d"));

    let buckets: Vec<&str> = [6302, 6303, 6304]
        .iter()
        .map(|id| catalog.find(Coin::new(*id, OutcomeSide::Yes)).unwrap().title.as_str())
        .collect();
    assert_eq!(buckets, ["BTC below 81479", "BTC 81479 to 84804", "BTC above 84804"]);
    let other = catalog.find(coin("#63010")).unwrap();
    assert_eq!((other.kind, other.question), (Family::Fallback, Some(356)));
    let range = &catalog.questions[catalog.place_of(356).unwrap()];
    assert_eq!(range.title, "BTC price range at 2026-09-29 06:00Z");
    assert_eq!((range.legs.len(), range.fallback), (3, Some(coin("#63010"))));

    let league = &catalog.questions[catalog.place_of(198).unwrap()];
    assert_eq!(league.title, "English Premier League 2026/2027 winner");
    assert_eq!(league.legs.len(), 6);
    let order: Vec<u32> = catalog.markets.iter().map(|market| market.outcome).collect();
    assert_eq!(
        order,
        [6297, 6302, 6303, 6304, 6301, 4667, 1210, 1209, 1473, 1474, 1475, 1476, 1477, 1478, 1472],
        "soonest to expire first, then by kind, then by coin"
    );

    let later = Catalog::build(&meta, Stamp::parse("20260929-0615").unwrap().ms().unsigned_abs());
    assert_eq!(later.find(coin("#46670")).unwrap().status, Status::Expired);
}

#[test]
fn testnet_s_catalogue_reads_from_its_own_ids() {
    let meta: OutcomeMeta = serde_json::from_str(TESTNET).unwrap();
    let catalog = Catalog::build(&meta, TESTNET_NOW);
    let daily = catalog.find(coin("#231360")).unwrap();
    assert_eq!(daily.title, "BTC above 83136 at 2026-10-09 03:00Z");
    assert_eq!(daily.status, Status::Open);
    let buckets: Vec<&str> = [23150, 23151, 23152]
        .iter()
        .map(|id| catalog.find(Coin::new(*id, OutcomeSide::Yes)).unwrap().title.as_str())
        .collect();
    assert_eq!(buckets, ["BTC below 80907", "BTC 80907 to 84210", "BTC above 84210"]);

    let cup = &catalog.questions[catalog.place_of(823).unwrap()];
    assert_eq!((cup.kind, cup.title.as_str()), (Family::Named, "2026 World Cup champion"));
    assert_eq!(cup.legs, vec![coin("#102330"), coin("#102360")]);
    assert_eq!((cup.settled_legs, cup.fallback), (2, Some(coin("#102310"))));
    let argentina = catalog.find(coin("#102330")).unwrap();
    assert_eq!((argentina.kind, argentina.title.as_str()), (Family::Named, "Argentina"));
    assert_eq!(argentina.headline(), "2026 World Cup champion / Argentina");
    assert_eq!(catalog.find(coin("#102310")).unwrap().kind, Family::Fallback, "testnet spells it Fallback");

    let old = catalog.find(coin("#110100")).unwrap();
    assert_eq!((old.title.as_str(), old.venue.as_deref()), ("BTC above 100 at 2026-09-01 12:00Z", Some("ag")));
    assert_eq!(old.status, Status::Expired, "past its time and not yet settled");
}
