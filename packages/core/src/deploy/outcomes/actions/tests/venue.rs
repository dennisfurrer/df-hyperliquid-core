//! A venue name as typed: what is claimed as given and what is refused.

use super::*;
use crate::deploy::outcomes::actions::support::venue;

#[test]
fn a_venue_is_two_to_four_lower_case_letters_and_never_spot() {
    for name in ["ab", "out", "zzz", "abcd"] {
        assert_eq!(venue(name).as_str(), name);
    }
    for refused in ["", "a", "abcde", "AB", "Ab", "a1", "a b", " ab", "\u{e9}t\u{e9}"] {
        assert_eq!(refused.parse::<Venue>(), Err(DeployError::Venue(refused.to_owned())), "{refused:?}");
    }
    assert_eq!(
        "spot".parse::<Venue>().unwrap_err().to_string(),
        "`spot` is the spot venue's name and cannot be claimed"
    );
}
