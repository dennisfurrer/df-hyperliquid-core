//! A builder's fee in tenths of a basis point: as the percent `approveBuilderFee` signs, and as the
//! bare number `maxBuilderFee` answers.

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
    let caps = [PERP_BUILDER_CAP, SPOT_BUILDER_CAP, OUTCOME_BUILDER_CAP].map(|cap| cap.to_string());
    assert_eq!(caps, ["0.1%", "1%", "1%"], "a tenth of a percent on perps, one percent elsewhere");
}

#[test]
fn max_builder_fee_answers_a_bare_number_of_tenths() {
    assert_eq!(serde_json::from_str::<BuilderFee>("880").unwrap(), BuilderFee::from_tenths(880));
    assert_eq!(serde_json::from_str::<BuilderFee>("0").unwrap(), BuilderFee::ZERO);
    assert!(serde_json::from_str::<BuilderFee>("\"0.88%\"").is_err());
}
