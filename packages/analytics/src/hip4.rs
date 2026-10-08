//! HIP-4 deployers: who lists what ([`venues()`]), the templates each lists from, and what each
//! earns. No endpoint reports a deployer's fees, so they are estimated two ways, each an
//! [`Estimate`](crate::Estimate) that says which: from the deployer's own ledger ([`earned`],
//! what its balance holds that no row explains) and from fills by the fee model ([`paid`]).

pub mod earned;
pub mod paid;
pub mod venues;

pub use earned::{Earned, LedgerDelta, LedgerRow, Moved, explained};
pub use paid::{Paid, Take, Takes, by_venue};
pub use venues::{First, Uses, Venue, templates_of, venues};
