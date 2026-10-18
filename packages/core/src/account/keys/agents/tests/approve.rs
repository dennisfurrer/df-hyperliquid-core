//! An approval under a name, until an expiry, or unnamed, as it is posted and signed.

use super::*;
use crate::account::fixtures::address;
use crate::account::keys::agents::name::split_valid_until;

const AGENT: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
const NONCE: u64 = 1_790_412_105_001;
/// 2026-10-08, when the fixtures were read.
const NOW: u64 = 1_791_480_940_000;
const DAY: u64 = 86_400_000;

#[test]
fn an_expiry_is_written_into_the_name_and_read_back_out_of_it() {
    let name = AgentName::new("desk").unwrap();
    let until = NOW + 30 * DAY;
    let approve = ApproveAgent::expiring(address(AGENT), &name, until, NOW, NONCE).unwrap();
    let wire = approve.wire(Network::Mainnet);
    assert!(wire.contains(&format!(r#""agentName":"desk valid_until {until}","#)), "{wire}");
    assert_eq!(approve.name(), Some(format!("desk valid_until {until}").as_str()));
    assert_eq!(split_valid_until(&format!("desk valid_until {until}")), ("desk", Some(until)));
    assert_eq!(split_valid_until("desk"), ("desk", None));
    assert_eq!(split_valid_until("desk valid_until soon"), ("desk valid_until soon", None));
}

#[test]
fn an_expiry_lies_ahead_and_within_a_hundred_and_eighty_days() {
    let name = AgentName::new("desk").unwrap();
    let agent = address(AGENT);
    assert!(ApproveAgent::expiring(agent, &name, NOW + MAX_AGENT_VALIDITY_MS, NOW, NONCE).is_ok());
    for until in [NOW, NOW - 1, NOW + MAX_AGENT_VALIDITY_MS + 1] {
        assert_eq!(
            ApproveAgent::expiring(agent, &name, until, NOW, NONCE),
            Err(BadExpiry { until, now: NOW }),
            "{until}"
        );
    }
    assert_eq!(MAX_AGENT_VALIDITY_MS, 180 * DAY);
}

#[test]
fn the_unnamed_agent_posts_a_null_name_and_signs_the_empty_one() {
    let agent = address(AGENT);
    let null = ApproveAgent::unnamed(agent, NONCE);
    let empty = ApproveAgent::new(agent, "", NONCE);
    for network in Network::ALL {
        assert!(null.wire(network).contains(r#""agentName":null,"#), "{network}");
        assert!(empty.wire(network).contains(r#""agentName":"","#), "{network}");
        assert_eq!(null.digest(network), empty.digest(network), "{network}: one approval, two spellings");
    }
    assert_eq!((null.name(), empty.name()), (None, None));
    assert_eq!(
        ApproveAgent::named(agent, &AgentName::new("desk").unwrap(), NONCE),
        ApproveAgent::new(agent, "desk", NONCE)
    );
}
