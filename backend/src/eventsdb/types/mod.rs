mod conversion;

use std::collections::HashMap;

use crate::utils::timestamp_now;
use aws_sdk_dynamodb::model::AttributeValue;
use serde::{Deserialize, Serialize};
use shared::{EventData, EventInfo, EventState, EventTokens, QuestionItem};

use self::conversion::{attributes_to_event, event_to_attributes};

use super::{event_key, Error};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Default)]
pub struct ApiEventInfo {
    pub tokens: EventTokens,
    pub data: EventData,
    #[serde(rename = "createTimeUnix")]
    pub create_time_unix: i64,
    #[serde(rename = "deleteTimeUnix")]
    pub delete_time_unix: i64,
    pub deleted: bool,
    #[serde(rename = "lastEditUnix")]
    pub last_edit_unix: i64,
    pub questions: Vec<QuestionItem>,
    pub state: EventState,
    pub premium_order: Option<String>,
}

impl From<ApiEventInfo> for EventInfo {
    fn from(val: ApiEventInfo) -> Self {
        Self {
            tokens: val.tokens,
            data: val.data,
            create_time_unix: val.create_time_unix,
            delete_time_unix: val.delete_time_unix,
            deleted: val.deleted,
            last_edit_unix: val.last_edit_unix,
            questions: val.questions,
            state: val.state,
            premium: val.premium_order.is_some(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct EventEntry {
    pub event: ApiEventInfo,
    pub version: usize,
    pub ttl: Option<i64>,
}

impl EventEntry {
    pub const fn new(event: ApiEventInfo, ttl: Option<i64>) -> Self {
        Self {
            event,
            version: 0,
            ttl,
        }
    }

    pub fn bump(&mut self) {
        self.version += 1;
        self.event.last_edit_unix = timestamp_now();
    }
}

pub type AttributeMap = HashMap<std::string::String, AttributeValue>;

const CURRENT_FORMAT: usize = 1;

impl TryFrom<&AttributeMap> for EventEntry {
    type Error = super::Error;

    fn try_from(value: &AttributeMap) -> Result<Self, Error> {
        let version = value["v"]
            .as_n()
            .map_err(|_| Error::General("malformed event: `v`".into()))?
            .parse::<usize>()?;

        let event = value["event"]
            .as_m()
            .map_err(|_| Error::MalformedObject("event".into()))?;

        let ttl = value
            .get("ttl")
            .and_then(|ttl| ttl.as_n().ok())
            .and_then(|ttl| ttl.parse::<i64>().ok());

        let event = attributes_to_event(event)?;

        Ok(Self {
            event,
            version,
            ttl,
        })
    }
}

impl From<EventEntry> for AttributeMap {
    fn from(value: EventEntry) -> Self {
        let mut map = Self::new();
        let event_key = event_key(&value.event.tokens.public_token);

        let event_av = event_to_attributes(value.event);
        let version_av = AttributeValue::N(value.version.to_string());
        let format_av = AttributeValue::N(CURRENT_FORMAT.to_string());
        let key_av = AttributeValue::S(event_key);

        map.insert("key".into(), key_av);
        map.insert("format".into(), format_av);
        map.insert("v".into(), version_av);
        map.insert("event".into(), AttributeValue::M(event_av));

        if let Some(ttl) = value.ttl {
            map.insert("ttl".into(), AttributeValue::N(ttl.to_string()));
        }

        map
    }
}

#[cfg(test)]
mod test_serialization {
    use super::*;
    use pretty_assertions::assert_eq;
    use shared::{EventState, States};

    #[test]
    fn test_ser_and_de_1() {
        // env_logger::init();

        let entry = EventEntry {
            event: ApiEventInfo {
                tokens: EventTokens {
                    public_token: String::from("token1"),
                    moderator_token: None,
                },
                data: EventData {
                    name: String::from("name"),
                    description: String::from("desc"),
                    short_url: String::from(""),
                    long_url: None,
                    mail: None,
                },
                create_time_unix: 1,
                delete_time_unix: 0,
                deleted: false,
                premium_order: Some(String::from("order")),
                last_edit_unix: 2,
                questions: vec![QuestionItem {
                    id: 0,
                    likes: 2,
                    text: String::from("q"),
                    hidden: false,
                    answered: true,
                    create_time_unix: 3,
                }],
                state: EventState {
                    state: States::Closed,
                },
            },
            version: 2,
            ttl: None,
        };

        let map: AttributeMap = entry.clone().try_into().unwrap();

        let entry_deserialized: EventEntry = (&map).try_into().unwrap();

        assert_eq!(entry, entry_deserialized);
    }

    #[test]
    fn test_ser_and_de_2() {
        // env_logger::init();

        let entry = EventEntry {
            event: ApiEventInfo {
                tokens: EventTokens {
                    public_token: String::from("token1"),
                    moderator_token: Some(String::from("token2")),
                },
                data: EventData {
                    name: String::from("name"),
                    description: String::from("desc"),
                    short_url: String::from(""),
                    long_url: Some(String::from("foo")),
                    mail: Some(String::from("mail")),
                },
                create_time_unix: 1,
                delete_time_unix: 0,
                deleted: false,
                premium_order: Some(String::from("order")),
                last_edit_unix: 2,
                questions: vec![QuestionItem {
                    id: 0,
                    likes: 2,
                    text: String::from("q"),
                    hidden: false,
                    answered: true,
                    create_time_unix: 3,
                }],
                state: EventState {
                    state: States::Closed,
                },
            },
            version: 2,
            ttl: Some(12345),
        };

        let map: AttributeMap = entry.clone().try_into().unwrap();

        let entry_deserialized: EventEntry = (&map).try_into().unwrap();

        assert_eq!(entry, entry_deserialized);
    }
}