//! What a named agent may be called.

use super::*;

#[test]
fn a_name_is_one_to_sixteen_characters_and_never_the_expiry_form() {
    // The builder's live agent's name is sixteen characters.
    for good in ["desk", "Desk-3712f660", "Builder Markets", "x"] {
        assert_eq!(AgentName::new(good).map(|name| name.as_str().to_owned()), Ok(good.to_owned()), "{good}");
    }
    for bad in ["", "   ", "seventeen chars!!", "desk valid_until 1"] {
        assert_eq!(AgentName::new(bad), Err(BadAgentName(bad.to_owned())), "{bad}");
    }
}
