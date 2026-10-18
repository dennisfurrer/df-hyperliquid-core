//! An approval under a name, until an expiry, or unnamed, as it is posted and signed.

use super::*;
use crate::account::fixtures::address;
use crate::account::keys::agents::name::split_valid_until;

const AGENT: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
const NONCE: u64 = 1_790_412_105_001;
/// 2026-10-08, when the fixtures were read.
const NOW: u64 = 1_791_480_940_000;
const DAY: u64 = 86_400_000;

/// An expiry lies ahead of now and at most 180 days ahead, and is written into the name, where the
/// venue reads it back out.
#[test]
fn an_expiry_lies_ahead_within_a_hundred_and_eighty_days_and_is_written_into_the_name() {
    let name = AgentName::new("desk").unwrap();
    let agent = address(AGENT);
    for (until, taken) in [
        (NOW + 30 * DAY, true),
        (NOW + MAX_AGENT_VALIDITY_MS, true),
        (NOW, false),
        (NOW - 1, false),
        (NOW + 180 * DAY + 1, false),
    ] {
        match ApproveAgent::expiring(agent, &name, until, NOW, NONCE) {
            Ok(approve) => {
                assert!(taken, "{until}");
                let written = format!("desk valid_until {until}");
                assert!(approve.wire(Network::Mainnet).contains(&format!(r#""agentName":"{written}","#)), "{until}");
                assert_eq!(
                    (approve.name(), split_valid_until(&written)),
                    (Some(written.as_str()), ("desk", Some(until)))
                );
            }
            Err(refused) => assert_eq!((refused, taken), (BadExpiry { until, now: NOW }, false), "{until}"),
        }
    }
    assert_eq!(split_valid_until("desk"), ("desk", None));
    assert_eq!(split_valid_until("desk valid_until soon"), ("desk valid_until soon", None));
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
    assert_eq!((null.name(), empty.name(), null.agent()), (None, None, agent));
    assert_eq!(
        ApproveAgent::named(agent, &AgentName::new("desk").unwrap(), NONCE),
        ApproveAgent::new(agent, "desk", NONCE)
    );
}
