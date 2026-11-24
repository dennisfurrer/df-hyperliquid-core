//! One outcome settled at a fraction, its words copied from `outcomeMeta`: the body of
//! `settleOutcome` and each entry of `settleQuestion2`.

use crate::deploy::outcomes::actions::bounded::Fraction;
use crate::utils::sign::{Map, Value};

/// One outcome settled: the body of `settleOutcome`, and each entry of
/// `settleQuestion2`. The name, description and side names must be the ones
/// `outcomeMeta` lists, character for character, trailing spaces included
/// ([`Outcome::settlement`](crate::markets::outcomes::Outcome::settlement) copies them); they are sent as
/// given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// The outcome's id.
    pub outcome: u32,
    /// The share the first side is paid (`settleFraction`).
    pub fraction: Fraction,
    /// The outcome's name as `outcomeMeta` lists it, sent first in `nameAndDescription`.
    pub name: String,
    /// The outcome's description as `outcomeMeta` lists it, sent second in `nameAndDescription`.
    pub description: String,
    /// The first side is the one `fraction` pays.
    pub sides: [String; 2],
}

impl Settlement {
    /// `details` is always empty: the venue refuses anything else.
    #[must_use]
    pub fn wire(&self) -> Map {
        let [first, second] = &self.sides;
        Map::new()
            .put("outcome", self.outcome)
            .put("settleFraction", self.fraction.to_wire())
            .put("details", "")
            .put("nameAndDescription", Value::array([self.name.clone(), self.description.clone()]))
            .put("sideNames", Value::array([first.clone(), second.clone()]))
    }
}
