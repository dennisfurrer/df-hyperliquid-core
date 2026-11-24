//! `activateOutcomeDeployer`: claiming a venue name and staking HYPE for it, or giving the venue up.

use crate::deploy::outcomes::actions::venue::Venue;
use crate::utils::sign::{L1Action, Map, Value};

/// `activateOutcomeDeployer`, which only the account's own key may sign, never an agent. Activating claims the
/// venue name for good and stakes HYPE for at least 183 days; deactivating needs those days behind it and no
/// live outcomes, and cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActivateOutcomeDeployer {
    /// `{"activate":{"venueName"}}`.
    Activate(Venue),
    /// `{"deactivate":null}`: the key is sent, with nil.
    Deactivate,
}

impl L1Action for ActivateOutcomeDeployer {
    fn action(&self) -> Map {
        let map = Map::new().put("type", "activateOutcomeDeployer");
        match self {
            ActivateOutcomeDeployer::Activate(venue) => {
                map.put("activate", Map::new().put("venueName", venue.as_str().to_owned()))
            }
            ActivateOutcomeDeployer::Deactivate => map.put("deactivate", Value::Nil),
        }
    }
}
