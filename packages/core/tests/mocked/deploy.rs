//! Deploying through the [`Client`]: the deploy auctions read before a bid, and a HIP-4 venue run
//! end to end: claimed, an outcome registered from a template its keywords were checked against,
//! a question settled with the words `outcomeMeta` lists, and settling handed to a sub-deployer.
//! Every action is L1 and recovers to the key that should have signed it.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::deploy::outcomes::{
    ActivateOutcomeDeployer, DeployError, FeeScale, Fraction, Grant, Instance, Keywords, OutcomeDeploy, Permission,
};
use df_hyperliquid_core::deploy::{outcomes, perp_deploy_auction, spot_deploy_state, spot_pair_deploy_auction};
use df_hyperliquid_core::markets::outcomes::{deployer_limits, outcome_meta, outcome_templates};
use df_hyperliquid_core::utils::{Address, Json};
use df_hyperliquid_core::{Client, Dec, Network, SendError};

use crate::support::venue::Venue;
use crate::support::{NOW, clock, key};

/// HLP Strategy A, deploying nothing when `spot_deploy_state_mainnet.json` was recorded.
const DEPLOYER: &str = "0x010461c14e146ac35fe42271bdc1134ee31c703a";
/// Hardhat's account #1, standing in for a sub-deployer: a published test key that holds nothing.
const SUB_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn compact(reads: &[Json]) -> Vec<String> {
    reads.iter().map(Json::compact).collect()
}

fn out() -> outcomes::Venue {
    "out".parse().unwrap()
}

fn settlements(action: &Json) -> Vec<(u64, String)> {
    let question = action.get("operation").and_then(|operation| operation.get("settleQuestion2")).unwrap();
    let legs = question.get("outcomeSettlements").and_then(Json::as_array).unwrap();
    legs.iter()
        .map(|leg| {
            let fraction = leg.get("settleFraction").and_then(Json::as_str).unwrap();
            (leg.get("outcome").and_then(Json::as_u64).unwrap(), fraction.to_owned())
        })
        .collect()
}

/// At [`NOW`] the spot token and spot pair rounds run at their 500 HYPE floor; the perp round ended,
/// sold at 541.69363865 HYPE.
#[tokio::test]
async fn a_deployer_reads_every_deploy_auction_and_which_of_them_runs_now() {
    let venue = Venue::mainnet()
        .info("spotDeployState", "spot/spot_deploy_state_mainnet.json")
        .info("spotPairDeployAuctionStatus", "spot/spot_pair_deploy_auction_mainnet.json")
        .info("perpDeployAuctionStatus", "hip3/perp_deploy_auction_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let deployer: Address = DEPLOYER.parse().unwrap();
    let now = NOW / 1_000;

    let state = spot_deploy_state(&client, deployer).await.unwrap();
    let pair = spot_pair_deploy_auction(&client).await.unwrap();
    let perp = perp_deploy_auction(&client).await.unwrap();

    assert!(state.states.is_empty());
    let floor: Dec = "500".parse().unwrap();
    for (round, running) in [(&state.gas_auction, true), (&pair, true), (&perp, false)] {
        assert_eq!(round.is_running(now), running, "{round:?}");
        assert_eq!(round.duration_seconds, 31 * 3_600, "{round:?}");
    }
    assert_eq!((state.gas_auction.current_gas, pair.current_gas), (Some(floor), Some(floor)));
    assert_eq!((perp.current_gas, perp.end_gas), (None, Some("541.69363865".parse().unwrap())));
    assert_eq!(
        compact(&client.http().reads()),
        [
            format!(r#"{{"type":"spotDeployState","user":"{DEPLOYER}"}}"#),
            r#"{"type":"spotPairDeployAuctionStatus"}"#.to_owned(),
            r#"{"type":"perpDeployAuctionStatus"}"#.to_owned(),
        ]
    );
}

/// Venue `out` as mainnet listed it: question 198 of six clubs and a fallback, and two standalone
/// price outcomes. Settling pays out at once, so what `outcomeMeta` does not allow is refused
/// before anything is signed.
#[tokio::test]
async fn an_outcome_is_registered_from_its_template_and_a_question_settled_from_outcome_meta() {
    let venue = Venue::mainnet()
        .info("outcomeTemplates", "hip4/outcome_templates_mainnet.json")
        .info("outcomeDeployerLimits", "hip4/deployer_limits_out_mainnet.json")
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .exchange("default")
        .exchange("default")
        .exchange("refused_whole")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let templates = outcome_templates(&client).await.unwrap();
    let binary = templates.iter().find(|template| template.id == "binaryPrice").unwrap();
    let typed = Keywords::from_pairs([
        ("time", "20261231-1200"),
        ("threshold", "130000"),
        ("seconds", "60"),
        ("priceDescription", "BTC-USDC perp mark"),
        ("perp", "BTC"),
    ])
    .unwrap();
    assert_eq!(typed.check(binary.keywords.iter().map(|keyword| (keyword.name.as_str(), keyword.hint())), NOW), []);
    let room = deployer_limits(&client, "out").await.unwrap();
    assert!(room.daily > 0 && room.active > 0, "{room:?}");
    let scale: FeeScale = "1.0".parse().unwrap();
    client.send(&OutcomeDeploy::register_standalone(out(), Instance::new("binaryPrice", typed), scale)).await.unwrap();
    let meta = outcome_meta(&client).await.unwrap();
    client.send(&OutcomeDeploy::settle_question(out(), &meta, 198, 1474).unwrap()).await.unwrap();
    assert_eq!(
        OutcomeDeploy::settle_outcome(out(), &meta, 1472, Fraction::ONE),
        Err(DeployError::Fallback { outcome: 1472, question: 198 })
    );
    assert_eq!(
        OutcomeDeploy::settle_outcome(out(), &meta, 1474, "0.5".parse().unwrap()),
        Err(DeployError::NotWhole(1474))
    );
    let touched = OutcomeDeploy::settle_outcome(out(), &meta, 1209, Fraction::ONE).unwrap();
    let refused = client.send(&touched).await;

    assert_eq!(
        compact(&client.http().reads()),
        [
            r#"{"type":"outcomeTemplates"}"#,
            r#"{"type":"outcomeDeployerLimits","venue":"out"}"#,
            r#"{"type":"outcomeMeta"}"#
        ]
    );
    let actions = client.http().actions();
    let [register, question, touch] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        register.action.compact(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"registerStandaloneOutcomeFromTemplate":{"id":"binaryPrice","keywordToValue":[["perp","BTC"],["priceDescription","BTC-USDC perp mark"],["seconds","60"],["threshold","130000"],["time","20261231-1200"]],"deployerFeeScale":"1"}}}"#
    );
    assert_eq!(
        settlements(&question.action),
        [(1473, "0"), (1474, "1"), (1475, "0"), (1476, "0"), (1477, "0"), (1478, "0")]
            .map(|(outcome, fraction)| (outcome, fraction.to_owned())),
        "every named club, Manchester City at 1, the fallback left out"
    );
    assert_eq!(
        touch.action.compact(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleOutcome":{"outcome":1209,"settleFraction":"1","details":"","nameAndDescription":["template:priceTouch","perp:HYPE|priceDescription:HYPE-USDC perp mark |seconds:1|target:100|time:20261001-0000"],"sideNames":["template:Yes","template:No"]}}}"#,
        "the listed words exactly, the trailing space too"
    );
    assert!(actions.iter().all(|sent| sent.signer == Some(key().address()) && sent.vault.is_none()));
    assert_eq!(refused, Err(SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))));
}

/// The deployer claims `out` and lets one address settle standalone outcomes; that address then
/// settles BTC's 100000 threshold, which the 1 October close did not reach, under its own key.
#[tokio::test]
async fn a_deployer_claims_its_venue_and_a_sub_deployer_it_granted_settles_for_it() {
    let sub = LocalKey::from_hex(SUB_KEY).unwrap();
    let granted = Grant { permission: Permission::SettleOutcome, user: sub.address(), allowed: true };
    let venue = Venue::mainnet().exchange("default").exchange("default").signed_by(key().address());
    let deployer = Client::new(venue, Network::Mainnet, clock, key());
    deployer.send(&ActivateOutcomeDeployer::Activate(out())).await.unwrap();
    deployer.send(&OutcomeDeploy::set_sub_deployers(out(), vec![granted]).unwrap()).await.unwrap();

    let venue = Venue::mainnet()
        .info("outcomeMeta", "hip4/outcome_meta_mainnet.json")
        .exchange("default")
        .signed_by(sub.address());
    let settler = Client::new(venue, Network::Mainnet, clock, sub);
    let meta = outcome_meta(&settler).await.unwrap();
    let settle = OutcomeDeploy::settle_outcome(out(), &meta, 1210, Fraction::ZERO).unwrap();
    assert_eq!(settle.operation.permission(), Some(granted.permission));
    settler.send(&settle).await.unwrap();

    let claimed: Vec<String> = deployer.http().actions().iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        claimed,
        [
            r#"{"type":"activateOutcomeDeployer","activate":{"venueName":"out"}}"#.to_owned(),
            format!(
                r#"{{"type":"outcomeDeploy","venue":"out","operation":{{"setSubDeployers":[{{"variant":"settleOutcome","user":"{}","allowed":true}}]}}}}"#,
                granted.user
            ),
        ]
    );
    let actions = settler.http().actions();
    let [settled] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        settled.action.compact(),
        r#"{"type":"outcomeDeploy","venue":"out","operation":{"settleOutcome":{"outcome":1210,"settleFraction":"0","details":"","nameAndDescription":["template:binaryPrice","perp:BTC|priceDescription:BTC-USDC mark|seconds:1|threshold:100000|time:20261001-0000"],"sideNames":["template:Yes","template:No"]}}}"#
    );
    assert_eq!(settled.signer, Some(granted.user));
}
