//! The list of plain channels and the table that reads them name the same channels.

use super::*;
use crate::live::fixtures::all;

/// Asks [`plain`] about a channel without any data: it decides by the name alone.
fn read_by_plain(channel: &str) -> bool {
    plain(channel, serde::de::value::UnitDeserializer::<serde::de::value::Error>::new()).is_some()
}

#[test]
fn the_list_and_the_table_name_the_same_channels() {
    for channel in PLAIN {
        assert!(read_by_plain(channel), "{channel} is listed but not read");
    }
    for (name, text) in all() {
        let Some(channel) = channel_of(&text) else { continue };
        assert_eq!(is_plain(channel), read_by_plain(channel), "{name}: {channel}");
    }
    for channel in ["pong", "subscriptionResponse", "post", "outcomeMetaUpdates", "somethingNew"] {
        assert!(!is_plain(channel) && !read_by_plain(channel), "{channel}");
    }
}

#[test]
fn the_channel_is_read_only_from_the_venue_s_own_start() {
    assert_eq!(channel_of(r#"{"channel":"l2Book","data":{}}"#), Some("l2Book"));
    assert_eq!(channel_of(r#"{"channel":"pong"}"#), Some("pong"));
    assert_eq!(channel_of(r#"{ "channel":"l2Book"}"#), None);
    assert_eq!(channel_of(r#"{"data":{},"channel":"l2Book"}"#), None);
    let escaped = format!("{{\"channel\":\"l2{}u0042ook\"}}", char::from(0x5c));
    assert_eq!(channel_of(&escaped), None);
}
