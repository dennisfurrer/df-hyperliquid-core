//! A fee scale and a settle fraction as typed: the range each keeps and the form each is sent in.

use super::*;

#[test]
fn a_fee_scale_and_a_fraction_are_in_range_and_sent_in_normal_form() {
    for (typed, sent) in [
        ("0", "0"),
        ("1", "1"),
        ("1.0", "1"),
        ("01", "1"),
        (".50", "0.5"),
        ("2.25", "2.25"),
        ("10", "10"),
        ("10.000", "10"),
    ] {
        let scale: FeeScale = typed.parse().unwrap();
        assert_eq!(scale.to_wire(), sent, "{typed}");
        assert_eq!(scale.to_string(), sent);
    }
    for refused in ["10.01", "11", "-1", "1e1", "", "abc", "+1", "1,5"] {
        assert_eq!(refused.parse::<FeeScale>(), Err(DeployError::FeeScale(refused.to_owned())), "{refused}");
    }
    assert_eq!(
        "10.01".parse::<FeeScale>().unwrap_err().to_string(),
        "a fee scale is a decimal from 0 to 10, which `10.01` is not"
    );
    assert_eq!(FeeScale::ONE, "1.0".parse::<FeeScale>().unwrap());

    for (typed, sent, whole) in [
        ("0", "0", true),
        ("1", "1", true),
        ("1.0", "1", true),
        ("0.000", "0", true),
        ("0.50", "0.5", false),
        (".25", "0.25", false),
        ("0.999999", "0.999999", false),
    ] {
        let fraction: Fraction = typed.parse().unwrap();
        assert_eq!(fraction.to_wire(), sent, "{typed}");
        assert_eq!(fraction.is_whole(), whole, "{typed}");
    }
    for refused in ["1.01", "2", "-0.5", "0.5e0", "", "one"] {
        assert_eq!(refused.parse::<Fraction>(), Err(DeployError::Fraction(refused.to_owned())), "{refused}");
    }
    assert_eq!(Fraction::ONE, "1.000".parse::<Fraction>().unwrap());
}
