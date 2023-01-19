/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::CommandContext,
    error::CliResult,
    tools::ledger::{parse_transaction_response, Ledger, Response, ResponseType},
    utils::table::print_table,
};

use crate::{
    error::CliError,
    tools::{pool::Pool, wallet::Wallet},
};
use indy_utils::did::DidValue;
use indy_vdr::pool::PreparedRequest;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

macro_rules! send_write_request {
    ($ctx:expr, $params:expr, $request:expr, $wallet:expr, $submitter_did:expr) => {{
        let sign = ParamParser::get_opt_bool_param("sign", $params)?
            .unwrap_or(super::super::constants::SIGN_REQUEST);
        let endorser = ParamParser::get_opt_did_param("endorser", $params)?;
        let mut send = ParamParser::get_opt_bool_param("send", $params)?
            .unwrap_or(super::super::constants::SEND_REQUEST);

        match endorser {
            Some(endorser_did) => {
                send = false;
                Ledger::append_request_endorser($request, &endorser_did).map_err(|err| {
                    println_err!("{}", err.message(None));
                })?
            }
            None => {}
        };

        if sign {
            Ledger::sign_request($wallet, $submitter_did, $request).map_err(|err| {
                println_err!("{}", err.message(None));
            })?;
        };

        send_request!($ctx, $params, $request, send)
    }};
}

macro_rules! send_read_request {
    ($ctx:expr, $params:expr, $request:expr) => {{
        let send = ParamParser::get_opt_bool_param("send", $params)?
            .unwrap_or(super::super::constants::SEND_REQUEST);
        send_request!($ctx, $params, $request, send)
    }};
}

macro_rules! send_request {
    ($ctx:expr, $params:expr, $request:expr, $send:expr) => {{
        if $send {
            let pool = $ctx.ensure_connected_pool()?;
            let response_json = Ledger::submit_request(&pool, $request).map_err(|err| {
                println_err!("{}", err.message(None));
            })?;

            let response = serde_json::from_str::<Response<JsonValue>>(&response_json)
                .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;

            (response_json, response)
        } else {
            let request_json = json!(&$request.req_json).to_string();
            println_succ!("Transaction has been created:");
            println!("     {:?}", request_json);
            $ctx.set_context_transaction(Some(request_json));
            return Ok(());
        }
    }};
}

macro_rules! get_transaction_to_use {
    ($ctx:expr, $param_txn:expr) => ({
        if let Some(txn_) = $param_txn {
            PreparedRequest::from_request_json(&txn_)
                .map_err(|_| println_err!("Invalid formatted transaction provided."))?
        } else if let Some(txn_) = $ctx.get_context_transaction() {
            println!("Transaction stored into context: {:?}.", txn_);
            println!("Would you like to use it? (y/n)");

            let use_transaction = crate::command_executor::wait_for_user_reply($ctx);

            if !use_transaction {
                println!("No transaction has been used.");
                return Ok(());
            }

            PreparedRequest::from_request_json(&txn_)
                .map_err(|_| println_err!("Invalid formatted transaction provided."))?
        } else {
            println_err!("There is not a transaction to use.");
            println!("You either need to explicitly pass transaction as a parameter, or \
                    load transaction using `ledger load-transaction`, or \
                    build a transaction (with passing either `send=false` or `endorser` parameter).");
            return Err(());
        }
    })
}

pub fn handle_transaction_response(response: Response<JsonValue>) -> Result<JsonValue, ()> {
    match response {
        Response {
            op: ResponseType::REPLY,
            result: Some(result),
            reason: None,
        } => Ok(result),
        Response {
            op: ResponseType::REQNACK,
            result: None,
            reason: Some(reason),
        }
        | Response {
            op: ResponseType::REJECT,
            result: None,
            reason: Some(reason),
        } => {
            println_err!("Transaction has been rejected: {}", reason);
            Err(())
        }
        _ => {
            println_err!("Invalid data has been received");
            Err(())
        }
    }
}

pub fn send_nym(
    ctx: &CommandContext,
    pool: &Pool,
    store: &Wallet,
    did: &DidValue,
    verkey: &str,
) -> CliResult<JsonValue> {
    let mut request = Ledger::build_nym_request(Some(&pool), did, did, Some(&verkey), None, None)?;

    if let Some((text, version, acc_mech_type, time_of_acceptance)) =
        ctx.get_transaction_author_info()
    {
        if acc_mech_type.is_empty() {
            return Err(CliError::InvalidEntityState(
                "Transaction author agreement Acceptance Mechanism isn't set.".to_string(),
            ));
        }

        Ledger::append_txn_author_agreement_acceptance_to_request(
            Some(&pool),
            &mut request,
            Some(&text),
            Some(&version),
            None,
            &acc_mech_type,
            time_of_acceptance,
        )?;
    };

    let response_json = Ledger::sign_and_submit_request(&pool, &store, &did, &mut request)?;
    let response: Response<serde_json::Value> =
        serde_json::from_str::<Response<serde_json::Value>>(&response_json)?;
    let response = handle_transaction_response(response)
        .map_err(|_| CliError::InvalidInput("".to_string()))?;
    Ok(response)
}

pub fn get_current_verkey(
    pool: &Pool,
    store: &Wallet,
    did: &DidValue,
) -> Result<Option<String>, ()> {
    //TODO: There nym is requested. Due to freshness issues response might be stale or outdated. Something should be done with it
    let response_json = Ledger::build_get_nym_request(Some(pool), Some(did), did)
        .and_then(|mut request| Ledger::sign_and_submit_request(pool, store, did, &mut request))
        .map_err(|err| println_err!("{}", err.message(Some(&pool.name))))?;
    let response: Response<serde_json::Value> =
        serde_json::from_str::<Response<serde_json::Value>>(&response_json)
            .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;
    let result = handle_transaction_response(response)?;
    let data = serde_json::from_str::<serde_json::Value>(&result["data"].as_str().unwrap_or("{}"))
        .map_err(|_| println_err!("Wrong data has been received"))?;
    let verkey = data["verkey"].as_str().map(String::from);
    Ok(verkey)
}

pub fn get_active_transaction_author_agreement(
    pool: &Pool,
) -> Result<Option<(String, String, Option<String>)>, ()> {
    let response = Ledger::build_get_txn_author_agreement_request(Some(pool), None, None)
        .and_then(|request| Ledger::submit_request(pool, &request))
        .map_err(|err| println_err!("{}", err.message(None)))?;

    let response = serde_json::from_str::<JsonValue>(&response)
        .map_err(|err| println_err!("Invalid transaction response: {:?}", err))?;

    let text = response["result"]["data"]["text"].as_str();
    let version = response["result"]["data"]["version"].as_str();
    let digest = response["result"]["data"]["digest"].as_str();

    match (text, version, digest) {
        (Some(text), _, _) if text.is_empty() => Ok(None),
        (Some(text), Some(version), digest) => Ok(Some((
            text.to_string(),
            version.to_string(),
            digest.as_ref().map(|digest_| digest_.to_string()),
        ))),
        _ => Ok(None),
    }
}

pub fn sign_and_submit_action(
    wallet: &Wallet,
    pool: &Pool,
    submitter_did: &DidValue,
    request: &mut PreparedRequest,
    nodes: Option<Vec<&str>>,
    timeout: Option<i64>,
) -> CliResult<String> {
    let nodes = match nodes {
        Some(n) => Some(json!(n).to_string()),
        None => None,
    };

    Ledger::sign_request(wallet, submitter_did, request)?;
    let replies =
        Ledger::submit_action(pool, &request, nodes.as_ref().map(String::as_ref), timeout)?;

    let replies: HashMap<String, String> = replies
        .into_iter()
        .map(|(node, reply)| (node, reply.to_string()))
        .collect();

    Ok(json!(replies).to_string())
}

pub fn set_author_agreement(ctx: &CommandContext, request: &mut PreparedRequest) -> Result<(), ()> {
    let pool = ctx.get_connected_pool();

    if let Some((text, version, acc_mech_type, time_of_acceptance)) =
        ctx.get_transaction_author_info()
    {
        if acc_mech_type.is_empty() {
            println_err!("Transaction author agreement Acceptance Mechanism isn't set.");
            return Err(());
        }

        Ledger::append_txn_author_agreement_acceptance_to_request(
            pool.as_deref(),
            request,
            Some(&text),
            Some(&version),
            None,
            &acc_mech_type,
            time_of_acceptance,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;
    };
    Ok(())
}

pub fn print_transaction_response(
    result: JsonValue,
    title: &str,
    data_sub_field: Option<&str>,
    data_headers: &[(&str, &str)],
    skip_empty: bool,
) {
    println_succ!("{}", title);

    let (metadata_headers, metadata, data) = match parse_transaction_response(result) {
        Ok(val) => val,
        Err(_) => return,
    };

    println_succ!("Metadata:");
    print_table(&metadata, &metadata_headers);

    let data = if data_sub_field.is_some() {
        &data[data_sub_field.unwrap()]
    } else {
        &data
    };
    let mut data_headers = data_headers.to_vec();
    if skip_empty {
        data_headers.retain(|&(ref key, _)| !data[key].is_null());
    }

    println_succ!("Data:");
    print_table(data, &data_headers);
}
