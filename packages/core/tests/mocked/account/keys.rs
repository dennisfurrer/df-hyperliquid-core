//! Who signs for an account: an agent approved only when the venue has never seen it, then signing
//! for its master, and a builder's fee approved before the builder is checked fit to be paid. The
//! owner's approvals are user-signed, so the venue checks the chain they were signed for; what an
//! agent signs must recover to the agent.

use df_hyperliquid_core::account::{
    AbstractionMode, ActionKind, AgentName, AgentSetAbstraction, ApproveAgent, ApproveBuilderFee, Key, LocalKey,
    NotReady, Readiness, Reused, Role, builder_readiness, extra_agents, fresh_agent, max_builder_fee, may_sign, role,
};
use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::trade::BuilderFee;
use df_hyperliquid_core::{Client, Network, SendError};

use super::{BUILDER, address, bodies, dec};
use crate::support::venue::Venue;
use crate::support::{NOW, clock, fixture, key};

/// Hardhat's account #1, which the venue knows as a user.
const KNOWN: &str = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
const DAY_MS: u64 = 86_400_000;

/// A key whose address (`0xc171...385e`) the venue has never seen.
fn agent_key() -> LocalKey {
    LocalKey::from_bytes(&[0x0a; 32]).unwrap()
}

#[tokio::test]
async fn an_agent_is_approved_only_when_the_venue_has_never_seen_it_then_signs_for_its_master() {
    let (owner, agent) = (key().address(), agent_key().address());
    let venue = Venue::mainnet()
        .info_where("userRole", &[("user", KNOWN)], &fixture("account/role_user_mainnet.json"))
        .info("userRole", "account/role_missing_mainnet.json")
        .info("extraAgents", "account/agents_none_mainnet.json")
        .exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let agents = Venue::mainnet().exchange("refused_whole").exchange("default").signed_by(agent);
    let as_agent = Client::new(agents, Network::Mainnet, clock, agent_key());

    let listed = extra_agents(&client, owner).await.unwrap();
    let known = role(&client, address(KNOWN)).await.unwrap();
    assert_eq!(fresh_agent(address(KNOWN), &known, &listed, &[]), Err(Reused::Known(Role::User)));
    let unseen = role(&client, agent).await.unwrap();
    assert_eq!(fresh_agent(agent, &unseen, &listed, &[]), Ok(()));

    let unified = AgentSetAbstraction::new(AbstractionMode::UnifiedAccount);
    let early = as_agent.send(&unified).await;
    assert_eq!(early, Err(SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))));

    let until = NOW + 30 * DAY_MS;
    let name = AgentName::new("desk").unwrap();
    let approve = ApproveAgent::expiring(agent, &name, until, NOW, client.next_nonce()).unwrap();
    client.send_user(&approve).await.unwrap();
    assert_eq!(may_sign(ActionKind::AgentSetAbstraction, Key::Agent), Ok(()));
    assert!(may_sign(ActionKind::ApproveBuilderFee, Key::Agent).is_err(), "refused before anything is signed");
    let accepted = as_agent.send(&unified).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            format!(r#"{{"type":"extraAgents","user":"{}"}}"#, owner.to_hex()),
            format!(r#"{{"type":"userRole","user":"{KNOWN}"}}"#),
            format!(r#"{{"type":"userRole","user":"{}"}}"#, agent.to_hex()),
        ]
    );
    let [approved] = client.http().actions().try_into().unwrap();
    assert_eq!(
        approved.action.compact(),
        format!(
            r#"{{"type":"approveAgent","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","agentAddress":"{}","agentName":"desk valid_until {until}","nonce":{NOW}}}"#,
            agent.to_hex()
        )
    );
    assert_eq!(approved.chain.as_deref(), Some("Mainnet"));
    let signed = as_agent.http().actions();
    assert_eq!(signed.len(), 2, "the refused one and the one taken");
    for sent in &signed {
        assert_eq!(sent.action.compact(), r#"{"type":"agentSetAbstraction","abstraction":"u"}"#);
        assert_eq!((sent.signer, sent.vault), (Some(agent), None));
    }
    assert_eq!(accepted.kind, "default");
}

#[tokio::test]
async fn a_builder_fee_is_approved_and_the_builder_checked_fit_to_be_paid() {
    let owner = key().address();
    let venue = Venue::mainnet()
        .info("maxBuilderFee", "account/max_builder_fee_mainnet.json")
        .info("clearinghouseState", "account/builder_perp.json")
        .info("userAbstraction", "account/mode_builder_mainnet.json")
        .exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let builder = address(BUILDER);

    let before = max_builder_fee(&client, owner, builder).await.unwrap();
    let approve = ApproveBuilderFee::new(builder, BuilderFee::from_tenths(100), client.next_nonce());
    client.send_user(&approve).await.unwrap();
    let ready = builder_readiness(&client, builder).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            format!(r#"{{"type":"maxBuilderFee","user":"{}","builder":"{BUILDER}"}}"#, owner.to_hex()),
            format!(r#"{{"type":"clearinghouseState","user":"{BUILDER}"}}"#),
            format!(r#"{{"type":"userAbstraction","user":"{BUILDER}"}}"#),
        ]
    );
    assert_eq!(before, BuilderFee::from_tenths(0), "never approved");
    let [approved] = client.http().actions().try_into().unwrap();
    assert_eq!(
        approved.action.compact(),
        format!(
            r#"{{"type":"approveBuilderFee","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","maxFeeRate":"0.1%","builder":"{BUILDER}","nonce":{NOW}}}"#
        )
    );
    assert_eq!(approve.max_fee(), Some(BuilderFee::from_tenths(100)), "0.1% is 100 tenths of a basis point");
    assert_eq!(ready, Readiness::NotReady(vec![NotReady::TooLittle { value: dec("0.003569") }]), "standard, but poor");
}
