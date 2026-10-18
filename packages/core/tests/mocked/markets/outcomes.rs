//! HIP-4 outcomes: a side bought off its book, a question titled from its templates, a settlement
//! read once the list drops it, and what a venue may still list. `outcomeMeta` is mainnet's on
//! 2026-10-18, trimmed to the 2026/2027 Premier League question and a recurring one.

use df_hyperliquid_core::markets::outcomes::{
    self, Book, Family, Fields, Mid, Px, Qty, Remaining, Template, TemplateRole, fill, label,
};
use df_hyperliquid_core::markets::{Dex, Precision, book, mids};
use df_hyperliquid_core::trade::{self, OrderAnswer, PlaceOrder, Side};
use df_hyperliquid_core::utils::OutcomeSide;
use df_hyperliquid_core::{Client, Network};

use super::{bodies, dec, sent_order};
use crate::support::venue::Venue;
use crate::support::{clock, fixture, key};

/// Arsenal to win the league: `#14730`'s book on 2026-09-28 and the mids on 2026-10-08. Twelve
/// dollars at the 0.58497 ask buys 20 contracts, bounded two percent over it on the grid.
#[tokio::test]
async fn an_outcome_side_is_bought_within_a_slippage_of_its_book() {
    let venue = Venue::mainnet()
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .info("l2Book", "hip4/l2_book_14730_mainnet.json")
        .info("allMids", "trade/all_mids.json")
        .exchange("order_filled")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let meta = outcomes::outcome_meta(&client).await.unwrap();
    let arsenal = meta.outcome(1473).unwrap();
    let yes = arsenal.coin(OutcomeSide::Yes);
    let read = book::l2_book(&client, &yes.to_string(), Precision::FULL).await.unwrap();
    let side = Book::try_from(read).unwrap();
    let (bid, ask) = side.top().unwrap();
    let reported = mids::all_mids(&client, Dex::Main).await.unwrap().get(&yes.to_string()).unwrap();
    let mid = Mid::parse(&reported.to_string()).unwrap();
    let size = Qty::spend(dec("12"), ask).unwrap();
    let order = yes.market_within(Side::Buy, ask, dec("0.02"), size).unwrap();
    let accepted = client.send(&PlaceOrder::one(order)).await.unwrap();

    assert_eq!(
        bodies(&client),
        [r#"{"type":"outcomeMeta"}"#, r##"{"type":"l2Book","coin":"#14730"}"##, r#"{"type":"allMids"}"#]
    );
    assert_eq!(meta.question_of(arsenal.outcome).map(|question| question.question), Some(198));
    assert_eq!((bid, ask), (Px::parse("0.58495").unwrap(), Px::parse("0.58497").unwrap()));
    assert_eq!(
        (mid.is_orderable(), mid.to_px_nearest()),
        (false, Px::parse("0.613").unwrap()),
        "0.612995 is between ticks"
    );
    assert!(size.worth_minimum(ask));
    assert_eq!(
        sent_order(&client),
        r#"{"a":100014730,"b":true,"p":"0.59666","s":"20","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}"#
    );
    assert_eq!(client.http().actions()[0].signer, Some(key().address()));
    let answers = trade::answer::orders(&accepted, 1).unwrap();
    assert!(matches!(answers.as_slice(), [OrderAnswer::Filled { .. }]), "{answers:?}");
}

/// Mainnet's templates on 2026-09-28: the question's title from its own keywords, each named
/// outcome's from its participant, and the sides as listed.
#[tokio::test]
async fn a_question_and_its_outcomes_are_titled_from_their_templates() {
    let venue = Venue::mainnet()
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .info("outcomeTemplates", "hip4/outcome_templates_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let meta = outcomes::outcome_meta(&client).await.unwrap();
    let templates = outcomes::outcome_templates(&client).await.unwrap();
    let template = |name: &str| {
        let id = name.strip_prefix("template:").unwrap();
        templates.iter().find(|template| template.id == id).unwrap()
    };
    let title = |name: &str, description: &str| {
        let fields = Fields::parse(description);
        fill(&template(name).name, |key| fields.text(key))
    };
    let question = meta.question(198).unwrap();
    let named: Vec<_> = question.unsettled().map(|outcome| meta.outcome(outcome).unwrap()).collect();

    assert_eq!(bodies(&client), [r#"{"type":"outcomeMeta"}"#, r#"{"type":"outcomeTemplates"}"#]);
    assert_eq!(Template::classify(&question.name).family, Family::TournamentWinner);
    assert_eq!(template(&question.name).role, TemplateRole::Question);
    assert_eq!(title(&question.name, &question.description), "2026/2027 English Premier League winner");
    let titles: Vec<String> = named.iter().map(|outcome| title(&outcome.name, &outcome.description)).collect();
    assert_eq!(
        titles,
        ["Arsenal", "Manchester City", "Liverpool", "Chelsea", "Manchester United", "Tottenham Hotspur"]
    );
    assert!(named.iter().all(|outcome| template(&outcome.name).parent() == Some("sportsTournamentWinner")));
    let fallback = meta.outcome(question.fallback_outcome.unwrap()).unwrap();
    assert_eq!(Template::classify(&fallback.name).family, Family::Fallback);
    let fields = Fields::parse(&named[0].description);
    let sides: Vec<String> = named[0].side_specs.iter().map(|side| label(Some(&side.name), &fields, "?")).collect();
    assert_eq!(sides, ["Yes", "No"]);
}

/// Outcome 6290 settled on 2026-09-28 and left the list; Arsenal's is still listed and answers
/// `null`.
#[tokio::test]
async fn a_settled_outcome_is_read_once_the_list_drops_it() {
    let venue = Venue::mainnet()
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .info_where("settledOutcome", &[("outcome", "6290")], &fixture("hip4/settled_outcome_6290_mainnet.json"))
        .info_text("settledOutcome", "null");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let meta = outcomes::outcome_meta(&client).await.unwrap();
    let settled = outcomes::settled_outcome(&client, 6290).await.unwrap().unwrap();
    let live = outcomes::settled_outcome(&client, 1473).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"outcomeMeta"}"#,
            r#"{"type":"settledOutcome","outcome":6290}"#,
            r#"{"type":"settledOutcome","outcome":1473}"#,
        ]
    );
    assert!(meta.outcome(6290).is_none() && meta.outcome(1473).is_some());
    assert_eq!((settled.spec.outcome, settled.winner()), (6290, Some(OutcomeSide::No)));
    assert_eq!((settled.pays(OutcomeSide::Yes), settled.pays(OutcomeSide::No)), (dec("0"), dec("1")));
    assert_eq!(live, None);
}

/// The `out` venue on 2026-10-18 holds its deployer in `outcomeMeta` and may list 793 more today.
#[tokio::test]
async fn a_venue_s_deployer_reads_how_many_more_outcomes_it_may_list() {
    let venue = Venue::mainnet()
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .info("outcomeDeployerLimits", "hip4/outcome_deployer_limits_out_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let meta = outcomes::outcome_meta(&client).await.unwrap();
    let venue = meta.outcome(1473).and_then(|arsenal| arsenal.venue.clone()).unwrap();
    let deployer = meta.deployer(&venue).unwrap();
    let remaining = outcomes::deployer_limits(&client, &venue).await.unwrap();

    assert_eq!(bodies(&client), [r#"{"type":"outcomeMeta"}"#, r#"{"type":"outcomeDeployerLimits","venue":"out"}"#]);
    assert_eq!(deployer.deployer.to_hex(), "0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e");
    assert_eq!(deployer.sub_deployers.len(), 5);
    assert_eq!(remaining, Remaining { daily: 793, active: 3 });
}
