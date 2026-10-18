//! `userEvents`, on the channel `user`: one kind of event a frame, a fill, a funding payment, a
//! liquidation, the venue's cancels or a TWAP's progress. A kind this build does not read is kept
//! as written.

use std::fmt;

use crate::account::TwapSliceFill;
use crate::account::history::Fill;
use crate::live::{Funding, TwapEvent};
use crate::utils::{Address, Dec};
use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

/// `userEvents`, on the channel `user`: one kind of event a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    Fills(Vec<Fill>),
    Funding(Funding),
    Liquidation(Liquidation),
    /// Orders the venue canceled, not the account.
    NonUserCancel(Vec<Cancel>),
    TwapHistory(Vec<TwapEvent>),
    TwapSliceFills(Vec<TwapSliceFill>),
    /// A kind this build does not read: its key, and its value as written.
    Other {
        key: String,
        data: String,
    },
}

impl<'de> Deserialize<'de> for UserEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Event;

        impl<'de> de::Visitor<'de> for Event {
            type Value = UserEvent;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object with one event in it")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<UserEvent, A::Error> {
                let key: String = map.next_key()?.ok_or_else(|| de::Error::custom("an event with nothing in it"))?;
                let event = match key.as_str() {
                    "fills" => UserEvent::Fills(map.next_value()?),
                    "funding" => UserEvent::Funding(map.next_value()?),
                    "liquidation" => UserEvent::Liquidation(map.next_value()?),
                    "nonUserCancel" => UserEvent::NonUserCancel(map.next_value()?),
                    "twapHistory" => UserEvent::TwapHistory(map.next_value()?),
                    "twapSliceFills" => UserEvent::TwapSliceFills(map.next_value()?),
                    _ => {
                        let data: Box<RawValue> = map.next_value()?;
                        UserEvent::Other { key, data: data.get().to_owned() }
                    }
                };
                if map.next_key::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("more than one event in a frame"));
                }
                Ok(event)
            }
        }

        deserializer.deserialize_map(Event)
    }
}

/// A liquidation the account took part in. The venue writes these keys in snake case.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Liquidation {
    pub lid: u64,
    pub liquidator: Address,
    pub liquidated_user: Address,
    pub liquidated_ntl_pos: Dec,
    pub liquidated_account_value: Dec,
}

/// An order the venue canceled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cancel {
    pub coin: String,
    pub oid: u64,
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
