//! A builder's fee in tenths of a basis point, as an order's `f` and `maxBuilderFee` count it (10
//! is 1 basis point). `approveBuilderFee` names the same figure as a percent.

use std::fmt;

use serde::Deserialize;

use crate::utils::dec::{Dec, Round};

/// Tenths of a basis point: 10 is 0.01%, 1000 is 1%.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize)]
#[serde(transparent)]
pub struct BuilderFee(u64);

/// The most a builder may take on a perp order, main dex or HIP-3: 0.1%.
pub const PERP_BUILDER_CAP: BuilderFee = BuilderFee(100);

/// The most a builder may take on a spot order: 1%.
pub const SPOT_BUILDER_CAP: BuilderFee = BuilderFee(1_000);

/// HIP-4 takes builder codes as spot does.
pub const OUTCOME_BUILDER_CAP: BuilderFee = SPOT_BUILDER_CAP;

impl BuilderFee {
    pub const ZERO: BuilderFee = BuilderFee(0);

    #[must_use]
    pub const fn from_tenths(tenths: u64) -> BuilderFee {
        BuilderFee(tenths)
    }

    #[must_use]
    pub const fn tenths(self) -> u64 {
        self.0
    }

    /// The percent string `approveBuilderFee` takes as `maxFeeRate`: 880 is `0.88%`, 100 is `0.1%`,
    /// 1000 is `1%`. It always matches `^[0-9]+(\.[0-9]+)?%$`.
    #[must_use]
    pub fn percent(self) -> String {
        let figure = Dec::lit::<3>(i128::from(self.0)).to_wire();
        format!("{figure}%")
    }

    /// A percent string back in tenths. `None` for anything that is not a
    /// whole number of tenths of a basis point.
    #[must_use]
    pub fn from_percent(text: &str) -> Option<BuilderFee> {
        let digits = text.trim().strip_suffix('%')?;
        if !digits.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
            return None;
        }
        let figure = Dec::parse(digits).ok()?;
        if figure.places() > 3 {
            return None;
        }
        u64::try_from(figure.units_at(3, Round::Down)?).ok().map(BuilderFee)
    }
}

impl fmt::Display for BuilderFee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.percent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fee_in_tenths_of_a_basis_point_reads_as_a_percent_and_back() {
        for (tenths, percent) in
            [(880, "0.88%"), (88, "0.088%"), (100, "0.1%"), (1000, "1%"), (10, "0.01%"), (1, "0.001%"), (0, "0%")]
        {
            assert_eq!(BuilderFee::from_tenths(tenths).percent(), percent, "{tenths}");
            assert_eq!(BuilderFee::from_percent(percent), Some(BuilderFee::from_tenths(tenths)), "{percent}");
        }
        assert_eq!(BuilderFee::from_percent("0.880%"), Some(BuilderFee::from_tenths(880)));
        for unreadable in ["0.0001%", "-1%", "+1%", "0.88", "", "%", "abc%", "1e1%"] {
            assert_eq!(BuilderFee::from_percent(unreadable), None, "{unreadable}");
        }
    }

    #[test]
    fn max_builder_fee_answers_a_bare_number_of_tenths() {
        let fee: BuilderFee = serde_json::from_str("880").unwrap();
        assert_eq!(fee, BuilderFee::from_tenths(880));
        assert_eq!(serde_json::from_str::<BuilderFee>("0").unwrap(), BuilderFee::ZERO);
        assert!(serde_json::from_str::<BuilderFee>("\"0.88%\"").is_err());
    }

    #[test]
    fn the_caps_are_a_tenth_of_a_percent_on_perps_and_one_percent_elsewhere() {
        assert_eq!((PERP_BUILDER_CAP.percent(), SPOT_BUILDER_CAP.percent()), ("0.1%".to_owned(), "1%".to_owned()));
        assert_eq!(OUTCOME_BUILDER_CAP, SPOT_BUILDER_CAP);
    }
}
