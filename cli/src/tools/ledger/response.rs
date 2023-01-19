/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::utils::time::timestamp_to_datetime;

use serde_json::Value as JsonValue;

#[derive(Deserialize, Eq, PartialEq, Debug)]
pub enum ResponseType {
    REQNACK,
    REPLY,
    REJECT,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Response<T> {
    pub op: ResponseType,
    pub reason: Option<String>,
    pub result: Option<T>,
}

pub fn parse_transaction_response(
    mut result: JsonValue,
) -> Result<(Vec<(&'static str, &'static str)>, JsonValue, JsonValue), ()> {
    match result["ver"].clone().as_str() {
        None => Ok(parse_transaction_response_v0(&mut result)),
        Some("1") => Ok(parse_transaction_response_v1(&mut result)),
        ver => Err(println_err!(
            "Unsupported transaction response format: {:?}",
            ver
        )),
    }
}

fn parse_transaction_response_v0(
    result: &mut JsonValue,
) -> (Vec<(&'static str, &'static str)>, JsonValue, JsonValue) {
    if let Some(txn_time) = result["txnTime"].as_i64() {
        result["txnTime"] = JsonValue::String(timestamp_to_datetime(txn_time))
    }

    let metadata_headers = vec![
        ("identifier", "Identifier"),
        ("seqNo", "Sequence Number"),
        ("reqId", "Request ID"),
        ("txnTime", "Transaction time"),
    ];

    (metadata_headers, result.clone(), result.clone())
}

fn parse_transaction_response_v1(
    result: &mut JsonValue,
) -> (Vec<(&'static str, &'static str)>, JsonValue, JsonValue) {
    if let Some(txn_time) = result["txnMetadata"]["txnTime"].as_i64() {
        result["txnMetadata"]["txnTime"] = JsonValue::String(timestamp_to_datetime(txn_time))
    }

    let mut metadata_headers = vec![
        ("from", "From"),
        ("seqNo", "Sequence Number"),
        ("reqId", "Request ID"),
        ("txnTime", "Transaction time"),
    ];

    let mut metadata_obj = result["txnMetadata"].as_object().unwrap().clone();

    metadata_obj.insert(
        "reqId".to_string(),
        result["txn"]["metadata"]["reqId"].clone(),
    );
    metadata_obj.insert(
        "from".to_string(),
        result["txn"]["metadata"]["from"].clone(),
    );

    if result["txn"]["metadata"]["endorser"].is_string() {
        metadata_headers.push(("endorser", "Endorser"));
        metadata_obj.insert(
            "endorser".to_string(),
            result["txn"]["metadata"]["endorser"].clone(),
        );
    }

    let metadata = JsonValue::Object(metadata_obj);
    let data = result["txn"]["data"].clone();

    (metadata_headers, metadata, data)
}
