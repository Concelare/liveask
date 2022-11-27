use async_trait::async_trait;
use aws_sdk_dynamodb::{
    error::{PutItemError, PutItemErrorKind},
    model::{
        AttributeDefinition, AttributeValue, KeySchemaElement, KeyType, ProvisionedThroughput,
        ScalarAttributeType,
    },
    types::SdkError,
};
use shared::EventInfo;
use tracing::instrument;

use crate::eventsdb::event_key;

use super::{
    error::{Error, Result},
    EventEntry, EventsDB,
};

#[derive(Clone)]
pub struct DynamoEventsDB {
    db: aws_sdk_dynamodb::Client,
    table: String,
}

#[async_trait]
impl EventsDB for DynamoEventsDB {
    #[instrument(skip(self), err)]
    async fn get(&self, key: &str) -> Result<EventEntry> {
        let key = event_key(key);

        let res = self
            .db
            .get_item()
            .table_name(&self.table)
            .key("key", AttributeValue::S(key))
            .send()
            .await?;

        let item = res.item().ok_or(Error::ItemNotFound)?;

        let version = item["v"]
            .as_n()
            .map_err(|_| Error::General("malformed event: v".into()))?
            .parse::<usize>()?;

        let value = item["value"]
            .as_s()
            .map_err(|_| Error::General("malformed event: value".to_string()))?;

        let event: EventInfo = serde_json::from_str(value)?;

        Ok(EventEntry { event, version })
    }

    #[instrument(skip(self), err)]
    async fn put(&self, event: EventEntry) -> Result<()> {
        let event_av = AttributeValue::S(serde_json::to_string(&event.event)?);
        let version_av = AttributeValue::N(event.version.to_string());
        let key_av = AttributeValue::S(event_key(&event.event.tokens.public_token));

        let mut request = self
            .db
            .put_item()
            .table_name(&self.table)
            .item("key", key_av)
            .item("v", version_av)
            .item("value", event_av);

        if event.version > 0 {
            let old_version_av = AttributeValue::N(event.version.saturating_sub(1).to_string());
            request = request
                .condition_expression("v = :ver")
                .expression_attribute_values(":ver", old_version_av);
        }

        //Note: filter out conditional error
        if let Err(e) = request.send().await {
            if matches!(&e,SdkError::<PutItemError>::ServiceError { err, .. }
            if matches!(
                err.kind,
                PutItemErrorKind::ConditionalCheckFailedException(_)
            )) {
                return Err(Error::Concurrency);
            }

            return Err(Error::DynamoPut(e));
        }

        Ok(())
    }
}

const DB_TABLE_NAME: &str = "liveask";

impl DynamoEventsDB {
    pub async fn new(db: aws_sdk_dynamodb::Client, check_table_exists: bool) -> Result<Self> {
        if check_table_exists {
            let resp = db.list_tables().send().await?;
            let names = resp.table_names().unwrap_or_default();

            tracing::trace!("tables: {}", names.join(","));

            if !names.contains(&DB_TABLE_NAME.into()) {
                tracing::info!("table not found, creating now");

                create_table(&db, DB_TABLE_NAME.into(), "key".into()).await?;
            }
        }

        Ok(Self {
            db,
            table: DB_TABLE_NAME.into(),
        })
    }
}

async fn create_table(
    client: &aws_sdk_dynamodb::Client,
    table_name: String,
    key_name: String,
) -> Result<()> {
    let ad = AttributeDefinition::builder()
        .attribute_name(&key_name)
        .attribute_type(ScalarAttributeType::S)
        .build();

    let ks = KeySchemaElement::builder()
        .attribute_name(&key_name)
        .key_type(KeyType::Hash)
        .build();

    let pt = ProvisionedThroughput::builder()
        .read_capacity_units(5)
        .write_capacity_units(5)
        .build();

    client
        .create_table()
        .table_name(table_name)
        .attribute_definitions(ad)
        .key_schema(ks)
        .provisioned_throughput(pt)
        .send()
        .await?;

    Ok(())
}
