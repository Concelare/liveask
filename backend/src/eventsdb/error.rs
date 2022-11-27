use std::num::ParseIntError;

use aws_sdk_dynamodb::{
    error::{CreateTableError, GetItemError, ListTablesError, PutItemError},
    types::SdkError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("General Error: {0}")]
    General(String),

    #[error("Concurrency Error")]
    Concurrency,

    #[error("Item Not Found")]
    ItemNotFound,

    #[error("Serde Error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("ParseInt Error: {0}")]
    ParseInt(#[from] ParseIntError),

    #[error("Dynamo PutItemError: {0}")]
    DynamoPut(#[from] SdkError<PutItemError>),

    #[error("Dynamo ListTablesError: {0}")]
    DynamoListTables(#[from] SdkError<ListTablesError>),

    #[error("Dynamo CreateTableError: {0}")]
    DynamoCreateTable(#[from] SdkError<CreateTableError>),

    #[error("Dynamo GetItemError: {0}")]
    DynamoGetItemError(#[from] SdkError<GetItemError>),
}

pub type Result<T> = std::result::Result<T, Error>;
