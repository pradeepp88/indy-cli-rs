extern crate chrono;
extern crate regex;

use crate::command_executor::{
    Command, CommandContext, CommandGroup, CommandGroupMetadata, CommandMetadata, CommandParams,
};
use crate::commands::*;
use crate::tools::ledger::Ledger;

use crate::error::CliResult;
use indy_vdr::pool::PreparedRequest;
use std::collections::{BTreeMap, HashMap};

use crate::utils::file::{read_file, write_file};
use crate::utils::table::{print_list_table, print_table};

use self::chrono::prelude::*;

use indy_utils::did::DidValue;
use indy_vdr::ledger::identifiers::{CredentialDefinitionId, SchemaId};
use indy_vdr::ledger::requests::cred_def::{
    CredentialDefinition, CredentialDefinitionData, CredentialDefinitionV1, SignatureType,
};
use indy_vdr::ledger::requests::node::{NodeOperationData, Services};
use indy_vdr::ledger::requests::schema::{AttributeNames, Schema, SchemaV1};
use serde_json::Value as JsonValue;

pub const SIGN_REQUEST: bool = true;
pub const SEND_REQUEST: bool = true;

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "ledger",
        "Ledger management commands"
    ));
}

macro_rules! send_write_request {
    ($ctx:expr, $params:expr, $request:expr, $wallet_handle:expr, $wallet_name:expr, $submitter_did:expr) => {{
        let sign = get_opt_bool_param("sign", $params)
            .map_err(error_err!())?
            .unwrap_or(SIGN_REQUEST);
        let endorser = get_opt_did_param("endorser", $params).map_err(error_err!())?;
        let mut send = get_opt_bool_param("send", $params)
            .map_err(error_err!())?
            .unwrap_or(SEND_REQUEST);

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
            Ledger::sign_request($wallet_handle, $submitter_did, $request).map_err(|err| {
                println_err!("{}", err.message(None));
            })?;
        };

        send_request!(
            $ctx,
            $params,
            $request,
            Some($wallet_name),
            Some($submitter_did),
            send
        )
    }};
}

macro_rules! send_read_request {
    ($ctx:expr, $params:expr, $request:expr, $submitter_did:expr) => {{
        let send = get_opt_bool_param("send", $params)
            .map_err(error_err!())?
            .unwrap_or(SEND_REQUEST);
        send_request!($ctx, $params, $request, None, $submitter_did, send)
    }};
}

macro_rules! send_request {
    ($ctx:expr, $params:expr, $request:expr, $wallet_name:expr, $submitter_did:expr, $send:expr) => {{
        if $send {
            let (pool, _) = ensure_connected_pool($ctx)?;
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
            set_transaction($ctx, Some(request_json));
            return Ok(());
        }
    }};
}

macro_rules! get_transaction_to_use {
    ($ctx:expr, $param_txn:expr) => ({
        if let Some(txn_) = $param_txn {
            PreparedRequest::from_request_json(&txn_)
                .map_err(|_| println_err!("Invalid formatted transaction provided."))?
        } else if let Some(txn_) = get_transaction($ctx) {
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

pub mod nym_command {
    use super::*;
    use crate::tools::did::Did;

    command!(CommandMetadata::build("nym", r#"Send NYM transaction to the Ledger."#)
                .add_required_param("did", "DID of new identity")
                .add_optional_param("verkey", "Verification key of new identity")
                .add_optional_param("role", "Role of identity. One of: STEWARD, TRUSTEE, TRUST_ANCHOR, ENDORSER, NETWORK_MONITOR or associated number, or empty in case of blacklisting NYM")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_optional_param("endorser","DID of the Endorser that will submit the transaction to the ledger. \
                    Note that specifying of this parameter implies send=false so the transaction will be prepared to pass to the endorser instead of sending to the ledger.\
                    The created request will be printed and stored into CLI context.")
                .add_example("ledger nym did=VsKV7grR1BUE29mG2Fm2kX")
                .add_example("ledger nym did=VsKV7grR1BUE29mG2Fm2kX verkey=GjZWsBLgZCR18aL468JAT7w9CZRiBnpxUPPgyQxh4voa")
                .add_example("ledger nym did=VsKV7grR1BUE29mG2Fm2kX role=TRUSTEE")
                .add_example("ledger nym did=VsKV7grR1BUE29mG2Fm2kX role=")
                .add_example("ledger nym did=VsKV7grR1BUE29mG2Fm2kX send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let pool = get_connected_pool(&ctx);
        let submitter_did = ensure_active_did(&ctx)?;

        let target_did = get_did_param("did", params).map_err(error_err!())?;
        let verkey = get_opt_str_param("verkey", params).map_err(error_err!())?;
        let role = get_opt_empty_str_param("role", params).map_err(error_err!())?;

        if let Some(target_verkey) = verkey {
            let did_info = Did::get_did_with_meta(&store, &target_did);

            if let Ok(ref did_info) = did_info {
                let verkey_ = Did::abbreviate_verkey(&did_info.did, &did_info.verkey)
                    .unwrap_or(did_info.verkey.to_string());

                if verkey_ != target_verkey {
                    println_warn!(
                    "There is the same `DID` stored in the wallet but with different Verkey: {:?}",
                    verkey_
                );
                    println_warn!("Do you really want to change Verkey on the ledger? (y/n)");

                    let change_nym = crate::command_executor::wait_for_user_reply(ctx);
                    if !change_nym {
                        println!("The transaction has not been sent.");
                        return Ok(());
                    }
                }
            }
        }

        let mut request = Ledger::build_nym_request(
            pool.as_deref(),
            &submitter_did,
            &target_did,
            verkey,
            None,
            role,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        set_author_agreement(ctx, &mut request)?;

        let (_, mut response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        if let Some(result) = response.result.as_mut() {
            result["txn"]["data"]["role"] = get_role_title(&result["txn"]["data"]["role"]);
            result["role"] = get_role_title(&result["role"]);
        }

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Nym request has been sent to Ledger.",
                None,
                &[("dest", "Did"), ("verkey", "Verkey"), ("role", "Role")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_nym_command {
    use super::*;

    command!(CommandMetadata::build("get-nym", "Get NYM from Ledger.")
                .add_required_param("did","DID of identity presented in Ledger")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-nym did=VsKV7grR1BUE29mG2Fm2kX")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let target_did = get_did_param("did", params).map_err(error_err!())?;

        let request =
            Ledger::build_get_nym_request(pool.as_deref(), submitter_did.as_ref(), &target_did)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        if let Some(result) = response.result.as_mut() {
            let data = serde_json::from_str::<JsonValue>(&result["data"].as_str().unwrap_or(""));
            match data {
                Ok(mut data) => {
                    data["role"] = get_role_title(&data["role"]);
                    result["data"] = data;
                }
                Err(_) => {
                    println_err!("NYM not found");
                    return Err(());
                }
            };
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Following NYM has been received.",
                Some("data"),
                &[
                    ("identifier", "Identifier"),
                    ("dest", "Dest"),
                    ("verkey", "Verkey"),
                    ("role", "Role"),
                ],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod attrib_command {
    use super::*;

    command!(CommandMetadata::build("attrib", r#"Send Attribute transaction to the Ledger for exists NYM."#)
                .add_required_param("did",  "DID of identity presented in Ledger")
                .add_optional_param("hash", "Hash of attribute data")
                .add_optional_param("raw", "JSON representation of attribute data")
                .add_optional_param("enc", "Encrypted attribute data")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_optional_param("endorser","DID of the Endorser that will submit the transaction to the ledger later. \
                    Note that specifying of this parameter implies send=false so the transaction will be prepared to pass to the endorser instead of sending to the ledger.\
                    The created request will be printed and stored into CLI context.")
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX raw={"endpoint":{"ha":"127.0.0.1:5555"}}"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX hash=83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX enc=aa3f41f619aa7e5e6b6d0d"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX raw={"endpoint":{"ha":"127.0.0.1:5555"}} send=false"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let pool = get_connected_pool(&ctx);
        let submitter_did = ensure_active_did(&ctx)?;

        let target_did = get_did_param("did", params).map_err(error_err!())?;
        let hash = get_opt_str_param("hash", params).map_err(error_err!())?;
        let raw = get_opt_object_param("raw", params).map_err(error_err!())?;
        let enc = get_opt_str_param("enc", params).map_err(error_err!())?;

        let mut request = Ledger::build_attrib_request(
            pool.as_deref(),
            &submitter_did,
            &target_did,
            hash,
            raw.as_ref(),
            enc,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        set_author_agreement(ctx, &mut request)?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        let attribute = if raw.is_some() {
            ("raw", "Raw value")
        } else if hash.is_some() {
            ("hash", "Hashed value")
        } else {
            ("enc", "Encrypted value")
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Attrib request has been sent to Ledger.",
                None,
                &[attribute],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_attrib_command {
    use super::*;

    command!(CommandMetadata::build("get-attrib", "Get ATTRIB from Ledger.")
                .add_required_param("did", "DID of identity presented in Ledger")
                .add_optional_param("raw", "Name of attribute")
                .add_optional_param("hash", "Hash of attribute data")
                .add_optional_param("enc", "Encrypted value of attribute data")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX raw=endpoint")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX hash=83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX enc=aa3f41f619aa7e5e6b6d0d")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let target_did = get_did_param("did", params).map_err(error_err!())?;
        let raw = get_opt_str_param("raw", params).map_err(error_err!())?;
        let hash = get_opt_str_param("hash", params).map_err(error_err!())?;
        let enc = get_opt_str_param("enc", params).map_err(error_err!())?;

        let request = Ledger::build_get_attrib_request(
            pool.as_deref(),
            submitter_did.as_ref(),
            &target_did,
            raw,
            hash,
            enc,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        if let Some(result) = response.result.as_mut() {
            let data = result["data"]
                .as_str()
                .map(|data| JsonValue::String(data.to_string()));
            match data {
                Some(data) => {
                    result["data"] = data;
                }
                None => {
                    println_err!("Attribute not found");
                    return Err(());
                }
            };
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Following ATTRIB has been received.",
                None,
                &[("data", "Data")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod schema_command {
    use super::*;

    command!(CommandMetadata::build("schema", r#"Send Schema transaction to the Ledger."#)
                .add_required_param("name", "Schema name")
                .add_required_param("version", "Schema version")
                .add_required_param("attr_names", "Schema attributes split by comma (the number of attributes should be less or equal than 125)")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_optional_param("endorser","DID of the Endorser that will submit the transaction to the ledger later. \
                    Note that specifying of this parameter implies send=false so the transaction will be prepared to pass to the endorser instead of sending to the ledger.\
                    The created request will be printed and stored into CLI context.")
                .add_example("ledger schema name=gvt version=1.0 attr_names=name,age")
                .add_example("ledger schema name=gvt version=1.0 attr_names=name,age send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let name = get_str_param("name", params).map_err(error_err!())?;
        let version = get_str_param("version", params).map_err(error_err!())?;
        let attr_names = get_str_array_param("attr_names", params).map_err(error_err!())?;

        let id = SchemaId::new(&submitter_did, name, version);
        let schema = Schema::SchemaV1(SchemaV1 {
            id,
            name: name.to_string(),
            version: version.to_string(),
            attr_names: AttributeNames::from(attr_names.as_slice()),
            seq_no: None,
        });

        let mut request = Ledger::build_schema_request(pool.as_deref(), &submitter_did, schema)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        set_author_agreement(ctx, &mut request)?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Schema request has been sent to Ledger.",
                Some("data"),
                &[
                    ("name", "Name"),
                    ("version", "Version"),
                    ("attr_names", "Attributes"),
                ],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_schema_command {
    use super::*;

    command!(CommandMetadata::build("get-schema", "Get Schema from Ledger.")
                .add_required_param("did", "DID of identity presented in Ledger")
                .add_required_param("name", "Schema name")
                .add_required_param("version", "Schema version")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-schema did=VsKV7grR1BUE29mG2Fm2kX name=gvt version=1.0")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let target_did = get_did_param("did", params).map_err(error_err!())?;
        let name = get_str_param("name", params).map_err(error_err!())?;
        let version = get_str_param("version", params).map_err(error_err!())?;

        let id = SchemaId::new(&target_did, name, version);

        let request =
            Ledger::build_get_schema_request(pool.as_deref(), submitter_did.as_ref(), &id)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        if let Some(result) = response.result.as_ref() {
            if !result["seqNo"].is_i64() {
                println_err!("Schema not found");
                return Err(());
            }
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Following Schema has been received.",
                Some("data"),
                &[
                    ("name", "Name"),
                    ("version", "Version"),
                    ("attr_names", "Attributes"),
                ],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_validator_info_command {
    use super::*;

    command!(
        CommandMetadata::build("get-validator-info", "Get validator info from all nodes.")
            .add_optional_param("nodes", "The list of node names to send the request")
            .add_optional_param("timeout", " Time to wait respond from nodes")
            .add_optional_param("timeout", " Time to wait respond from nodes")
            .add_example(r#"ledger get-validator-info"#)
            .add_example(r#"ledger get-validator-info nodes=Node1,Node2"#)
            .add_example(r#"ledger get-validator-info nodes=Node1,Node2 timeout=150"#)
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ensure_connected_pool_handle(&ctx)?;
        let store = ensure_opened_store(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;

        let nodes = get_opt_str_array_param("nodes", params).map_err(error_err!())?;
        let timeout = get_opt_number_param::<i64>("timeout", params).map_err(error_err!())?;

        let mut request = Ledger::build_get_validator_info_request(Some(&pool), &submitter_did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let response = if nodes.is_some() || timeout.is_some() {
            sign_and_submit_action(&store, &pool, &submitter_did, &mut request, nodes, timeout)
                .map_err(|err| println_err!("{}", err.message(None)))?
        } else {
            Ledger::sign_and_submit_request(&pool, &store, &submitter_did, &mut request)
                .map_err(|err| println_err!("{}", err.message(None)))?
        };

        let responses = match serde_json::from_str::<BTreeMap<String, String>>(&response) {
            Ok(responses) => responses,
            Err(_) => {
                let response = serde_json::from_str::<Response<JsonValue>>(&response)
                    .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;
                return handle_transaction_response(response)
                    .map(|result| println_succ!("{}", result));
            }
        };

        println_succ!("Validator Info:");

        let mut lines: Vec<String> = Vec::new();

        for (node, response) in responses {
            if response.eq("timeout") {
                lines.push(format!("\t{:?}: {:?}", node, "Timeout"));
                continue;
            }
            let response = match serde_json::from_str::<Response<JsonValue>>(&response) {
                Ok(resp) => resp,
                Err(err) => {
                    lines.push(format!(
                        "\t{:?}: \"Invalid data has been received: {:?}\"",
                        node, err
                    ));
                    continue;
                }
            };

            match handle_transaction_response(response) {
                Ok(result) => lines.push(format!("\t{:?}: {}", node, result)),
                Err(_) => {}
            };
        }

        println!("{{\n{}\n}}", lines.join(",\n"));

        trace!("execute <<");
        Ok(())
    }
}

pub mod cred_def_command {
    use super::*;

    command!(CommandMetadata::build("cred-def", r#"Send Cred Def transaction to the Ledger."#)
                .add_required_param("schema_id", "Sequence number of schema")
                .add_required_param("signature_type", "Signature type (only CL supported now)")
                .add_optional_param("tag", "Allows to distinct between credential definitions for the same issuer and schema. Note that it is mandatory for indy-node version 1.4.x and higher")
                .add_required_param("primary", "Primary key in json format")
                .add_optional_param("revocation", "Revocation key in json format")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_optional_param("endorser","DID of the Endorser that will submit the transaction to the ledger later. \
                    Note that specifying of this parameter implies send=false so the transaction will be prepared to pass to the endorser instead of sending to the ledger.\
                    The created request will be printed and stored into CLI context.")
                .add_example(r#"ledger cred-def schema_id=1 signature_type=CL tag=1 primary={"n":"1","s":"2","rms":"3","r":{"age":"4","name":"5"},"rctxt":"6","z":"7"}"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let schema_id = get_str_param("schema_id", params).map_err(error_err!())?;
        let signature_type = get_str_param("signature_type", params).map_err(error_err!())?;
        let tag = get_opt_str_param("tag", params)
            .map_err(error_err!())?
            .unwrap_or("");

        let primary = get_object_param("primary", params).map_err(error_err!())?;
        let revocation = get_opt_object_param("revocation", params).map_err(error_err!())?;

        let schema_id = SchemaId::from(schema_id.to_string());
        let id = CredentialDefinitionId::new(&submitter_did, &schema_id, signature_type, tag);

        let signature_type = SignatureType::from_str(signature_type)
            .map_err(|_| println_err!("Unsupported signature_type {}", signature_type))?;

        let cred_def = CredentialDefinition::CredentialDefinitionV1(CredentialDefinitionV1 {
            id,
            schema_id,
            signature_type,
            tag: tag.to_string(),
            value: CredentialDefinitionData {
                primary,
                revocation,
            },
        });

        let mut request = Ledger::build_cred_def_request(pool.as_deref(), &submitter_did, cred_def)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        set_author_agreement(ctx, &mut request)?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                Some("data"),
                &[("primary", "Primary Key"), ("revocation", "Revocation Key")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_cred_def_command {
    use super::*;

    command!(CommandMetadata::build("get-cred-def", "Get Cred Definition from Ledger.")
                .add_required_param("schema_id", "Sequence number of schema")
                .add_required_param("signature_type", "Signature type (only CL supported now)")
                .add_optional_param("tag", "Allows to distinct between credential definitions for the same issuer and schema. Note that it is mandatory for indy-node version 1.4.x and higher")
                .add_required_param("origin", "Credential definition owner DID")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-cred-def schema_id=1 signature_type=CL tag=1 origin=VsKV7grR1BUE29mG2Fm2kX")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let schema_id = get_str_param("schema_id", params).map_err(error_err!())?;
        let signature_type = get_str_param("signature_type", params).map_err(error_err!())?;
        let tag = get_opt_str_param("tag", params)
            .map_err(error_err!())?
            .unwrap_or("");
        let origin = get_did_param("origin", params).map_err(error_err!())?;

        let schema_id = SchemaId::from(schema_id.to_string());
        let id = CredentialDefinitionId::new(&origin, &schema_id, signature_type, tag);

        let request =
            Ledger::build_get_cred_def_request(pool.as_deref(), submitter_did.as_ref(), &id)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        if let Some(result) = response.result.as_ref() {
            if !result["seqNo"].is_i64() {
                println_err!("Credential Definition not found");
                return Err(());
            }
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Following Credential Definition has been received.",
                Some("data"),
                &[("primary", "Primary Key"), ("revocation", "Revocation Key")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod node_command {
    use super::*;

    command!(CommandMetadata::build("node", "Send Node transaction to the Ledger.")
                .add_required_param("target", "Node identifier")
                .add_required_param("alias", "Node alias (can't be changed in case of update)")
                .add_optional_param("node_ip", "Node Ip. Note that it is mandatory for adding node case")
                .add_optional_param("node_port", "Node port. Note that it is mandatory for adding node case")
                .add_optional_param("client_ip", "Client Ip. Note that it is mandatory for adding node case")
                .add_optional_param("client_port","Client port. Note that it is mandatory for adding node case")
                .add_optional_param("blskey",  "Node BLS key")
                .add_optional_param("blskey_pop",  "Node BLS key proof of possession. Note that it is mandatory if blskey specified")
                .add_optional_param("services", "Node type. One of: VALIDATOR, OBSERVER or empty in case of blacklisting node")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y node_ip=127.0.0.1 node_port=9710 client_ip=127.0.0.1 client_port=9711 alias=Node5 services=VALIDATOR blskey=2zN3bHM1m4rLz54MJHYSwvqzPchYp8jkHswveCLAEJVcX6Mm1wHQD1SkPYMzUDTZvWvhuE6VNAkK3KxVeEmsanSmvjVkReDeBEMxeDaayjcZjFGPydyey1qxBHmTvAnBKoPydvuTAqx5f7YNNRAdeLmUi99gERUU7TD8KfAa6MpQ9bw blskey_pop=RPLagxaR5xdimFzwmzYnz4ZhWtYQEj8iR5ZU53T2gitPCyCHQneUn2Huc4oeLd2B2HzkGnjAff4hWTJT6C7qHYB1Mv2wU5iHHGFWkhnTX9WsEAbunJCV2qcaXScKj4tTfvdDKfLiVuU2av6hbsMztirRze7LvYBkRHV3tGwyCptsrP")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y node_ip=127.0.0.1 node_port=9710 client_ip=127.0.0.1 client_port=9711 alias=Node5 services=VALIDATOR")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y alias=Node5 services=VALIDATOR")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y alias=Node5 services=")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let target_did = get_did_param("target", params).map_err(error_err!())?;
        let alias = get_str_param("alias", params).map_err(error_err!())?;
        let node_ip = get_opt_str_param("node_ip", params).map_err(error_err!())?;
        let node_port = get_opt_number_param::<i32>("node_port", params).map_err(error_err!())?;
        let client_ip = get_opt_str_param("client_ip", params).map_err(error_err!())?;
        let client_port =
            get_opt_number_param::<i32>("client_port", params).map_err(error_err!())?;
        let blskey = get_opt_str_param("blskey", params).map_err(error_err!())?;
        let blskey_pop = get_opt_str_param("blskey_pop", params).map_err(error_err!())?;
        let services = get_opt_str_array_param("services", params).map_err(error_err!())?;

        let services = match services {
            Some(services) => Some(
                services
                    .into_iter()
                    .map(|service| match service {
                        "VALIDATOR" => Ok(Services::VALIDATOR),
                        "OBSERVER" => Ok(Services::OBSERVER),
                        service => {
                            println_err!("Unsupported service \"{}\"!", service);
                            Err(())
                        }
                    })
                    .collect::<Result<Vec<Services>, ()>>()?,
            ),
            None => None,
        };

        let node_data = NodeOperationData {
            node_ip: node_ip.map(String::from),
            node_port,
            client_ip: client_ip.map(String::from),
            client_port,
            alias: alias.to_string(),
            services,
            blskey: blskey.map(String::from),
            blskey_pop: blskey_pop.map(String::from),
        };

        let mut request =
            Ledger::build_node_request(pool.as_deref(), &submitter_did, &target_did, node_data)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                Some("data"),
                &[
                    ("alias", "Alias"),
                    ("node_ip", "Node Ip"),
                    ("node_port", "Node Port"),
                    ("client_ip", "Client Ip"),
                    ("client_port", "Client Port"),
                    ("services", "Services"),
                    ("blskey", "Blskey"),
                    ("blskey_pop", "Blskey Proof of Possession"),
                ],
                true,
            )
        })?;
        trace!("execute <<");
        Ok(())
    }
}

pub mod pool_config_command {
    use super::*;

    command!(CommandMetadata::build("pool-config", "Send write configuration to pool.")
                .add_required_param("writes", "Accept write transactions.")
                .add_optional_param("force", "Forced configuration applying without reaching pool consensus.")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger pool-config writes=true")
                .add_example("ledger pool-config writes=true force=true")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let writes = get_bool_param("writes", params).map_err(error_err!())?;
        let force = get_opt_bool_param("force", params)
            .map_err(error_err!())?
            .unwrap_or(false);

        let mut request =
            Ledger::indy_build_pool_config_request(pool.as_deref(), &submitter_did, writes, force)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                None,
                &[("writes", "Writes"), ("force", "Force Apply")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod pool_restart_command {
    use super::*;

    command!(CommandMetadata::build("pool-restart", "Send instructions to nodes to restart themselves.")
                .add_required_param("action", "Restart type. Either start or cancel.")
                .add_optional_param("nodes","The list of node names to send the request")
                .add_optional_param("timeout"," Time to wait respond from nodes")
                .add_optional_param("datetime", "Node restart datetime (only for action=start).")
                .add_example(r#"ledger pool-restart action=start datetime=2020-01-25T12:49:05.258870+00:00"#)
                .add_example(r#"ledger pool-restart action=start datetime=2020-01-25T12:49:05.258870+00:00 nodes=Node1,Node2"#)
                .add_example(r#"ledger pool-restart action=start datetime=2020-01-25T12:49:05.258870+00:00 nodes=Node1,Node2 timeout=100"#)
                .add_example(r#"ledger pool-restart action=cancel"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (pool, pool_name) = ensure_connected_pool(&ctx)?;
        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;

        let action = get_str_param("action", params).map_err(error_err!())?;
        let datetime = get_opt_str_param("datetime", params).map_err(error_err!())?;
        let nodes = get_opt_str_array_param("nodes", params).map_err(error_err!())?;
        let timeout = get_opt_number_param::<i64>("timeout", params).map_err(error_err!())?;

        let mut request =
            Ledger::indy_build_pool_restart_request(Some(&pool), &submitter_did, action, datetime)
                .map_err(|err| println_err!("{}", err.message(Some(&pool_name))))?;

        let response = if nodes.is_some() || timeout.is_some() {
            sign_and_submit_action(&store, &pool, &submitter_did, &mut request, nodes, timeout)
                .map_err(|err| println_err!("{}", err.message(None)))?
        } else {
            Ledger::sign_and_submit_request(&pool, &store, &submitter_did, &mut request)
                .map_err(|err| println_err!("{}", err.message(None)))?
        };

        let responses = match serde_json::from_str::<HashMap<String, String>>(&response) {
            Ok(responses) => responses,
            Err(_) => {
                let response = serde_json::from_str::<Response<JsonValue>>(&response)
                    .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;
                return handle_transaction_response(response)
                    .map(|result| println_succ!("{}", result));
            }
        };

        for (node, response) in responses {
            if response.eq("timeout") {
                println_err!("Restart pool node {} timeout.", node);
                continue;
            }

            let response = serde_json::from_str::<Response<JsonValue>>(&response)
                .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;

            println_succ!("Restart pool response for node {}:", node);
            let _res = handle_transaction_response(response).map(|result| {
                print_table(
                    &result,
                    &[
                        ("identifier", "From"),
                        ("reqId", "Request Id"),
                        ("action", "Action"),
                        ("datetime", "Datetime"),
                    ],
                )
            });
        }

        trace!("execute <<");
        Ok(())
    }
}

pub mod pool_upgrade_command {
    use super::*;

    command!(CommandMetadata::build("pool-upgrade", "Send instructions to nodes to update themselves.")
                .add_required_param("name", "Human-readable name for the upgrade.")
                .add_required_param("version","The version of indy-node package we perform upgrade to. \n                  \
                                              Must be greater than existing one (or equal if reinstall flag is True)")
                .add_required_param("action", "Upgrade type. Either start or cancel.")
                .add_required_param("sha256", "Sha256 hash of the package.")
                .add_optional_param("timeout", "Limits upgrade time on each Node.")
                .add_optional_param("schedule", "Node upgrade schedule. Schedule should contain identifiers of all nodes. Upgrade dates should be in future. \n                              \
                                              If force flag is False, then it's required that time difference between each Upgrade must be not less than 5 minutes.\n                              \
                                              Requirements for schedule can be ignored by parameter force=true.\n                              \
                                              Schedule is mandatory for action=start.")
                .add_optional_param("justification", "Justification string for this particular Upgrade.")
                .add_optional_param("reinstall", "Whether it's allowed to re-install the same version. False by default.")
                .add_optional_param("force", "Whether we should apply transaction without waiting for consensus of this transaction. False by default.")
                .add_optional_param("package", "Package to be upgraded.")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=start sha256=f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398 schedule={"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00"}"#)
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=start sha256=f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398 schedule={"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00"} package=some_package"#)
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=cancel sha256=ac3eb2cc3ac9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let name = get_str_param("name", params).map_err(error_err!())?;
        let version = get_str_param("version", params).map_err(error_err!())?;
        let action = get_str_param("action", params).map_err(error_err!())?;
        let sha256 = get_str_param("sha256", params).map_err(error_err!())?;
        let timeout = get_opt_number_param::<u32>("timeout", params).map_err(error_err!())?;
        let schedule = get_opt_str_param("schedule", params).map_err(error_err!())?;
        let justification = get_opt_str_param("justification", params).map_err(error_err!())?;
        let reinstall = get_opt_bool_param("reinstall", params)
            .map_err(error_err!())?
            .unwrap_or(false);
        let force = get_opt_bool_param("force", params)
            .map_err(error_err!())?
            .unwrap_or(false);
        let package = get_opt_str_param("package", params).map_err(error_err!())?;

        let mut request = Ledger::indy_build_pool_upgrade_request(
            pool.as_deref(),
            &submitter_did,
            name,
            version,
            action,
            sha256,
            timeout,
            schedule,
            justification,
            reinstall,
            force,
            package,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        let mut schedule = None;
        let mut hash = None;
        if let Some(res) = response.result.as_ref() {
            schedule = res["schedule"].as_object().map(|s| {
                format!(
                    "{{{}\n}}",
                    s.iter()
                        .map(|(key, value)| format!(
                            "\n    {:?}:{:?}",
                            key,
                            value.as_str().unwrap_or("")
                        ))
                        .collect::<Vec<String>>()
                        .join(",")
                )
            });

            hash = res["sha256"].as_str().map(|h| h.to_string());
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                None,
                &[
                    ("name", "Name"),
                    ("action", "Action"),
                    ("version", "Version"),
                    ("timeout", "Timeout"),
                    ("justification", "Justification"),
                    ("reinstall", "Reinstall"),
                    ("force", "Force Apply"),
                    ("package", "Package Name"),
                ],
                true,
            )
        })?;
        if let Some(h) = hash {
            println_succ!("Hash:");
            println!("{}", h);
        }
        if let Some(s) = schedule {
            println_succ!("Schedule:");
            println!("{}", s);
        }
        trace!("execute <<");
        Ok(())
    }
}

pub mod custom_command {
    use super::*;

    command!(CommandMetadata::build("custom", "Send custom transaction to the Ledger.")
                .add_main_param("txn", "Transaction json. (Use \"context\" keyword to send a transaction stored into CLI context)")
                .add_optional_param("sign", "Is signature required")
                .add_example(r#"ledger custom {"reqId":1,"identifier":"V4SGRU86Z58d6TV7PBUe6f","operation":{"type":"105","dest":"V4SGRU86Z58d6TV7PBUe6f"},"protocolVersion":2}"#)
                .add_example(r#"ledger custom {"reqId":2,"identifier":"V4SGRU86Z58d6TV7PBUe6f","operation":{"type":"1","dest":"VsKV7grR1BUE29mG2Fm2kX"},"protocolVersion":2} sign=true"#)
                .add_example(r#"ledger custom context"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (pool, pool_name) = ensure_connected_pool(&ctx)?;

        let txn = get_str_param("txn", params).map_err(error_err!())?;
        let sign = get_opt_bool_param("sign", params)
            .map_err(error_err!())?
            .unwrap_or(false);

        let mut transaction = txn.to_string();

        if txn == "context" {
            let context_txn = get_transaction(ctx);

            match context_txn {
                Some(txn_) => {
                    println!("Transaction stored into context: {:?}.", txn_);
                    println!("Would you like to send it? (y/n)");

                    let use_transaction = crate::command_executor::wait_for_user_reply(ctx);

                    if !use_transaction {
                        println!("No transaction has been send.");
                        return Ok(());
                    }

                    transaction = txn_.to_string();
                }
                None => {
                    println_err!("There is not a transaction stored into CLI context.");
                    println!("You either need to load transaction using `ledger load-transaction`, or \
                        build a transaction (with passing a `send=false`) to store it into CLI context.");
                }
            }
        }

        let mut transaction = PreparedRequest::from_request_json(transaction)
            .map_err(|_| println_err!("Invalid formatted transaction provided."))?;

        let response = if sign {
            let (store, _) = ensure_opened_wallet(&ctx)?;
            let submitter_did = ensure_active_did(&ctx)?;
            Ledger::sign_and_submit_request(&pool, &store, &submitter_did, &mut transaction)
        } else {
            Ledger::submit_request(&pool, &transaction)
        };

        let response_json =
            response.map_err(|err| println_err!("{}", err.message(Some(&pool_name))))?;

        let response = serde_json::from_str::<Response<JsonValue>>(&response_json)
            .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;

        match response {
            Response {
                op: ResponseType::REPLY,
                result: Some(_),
                reason: None,
            } => {
                println!("Response: \n{}", response_json);
            }
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
                println_err!(
                    "Transaction has been rejected: {}",
                    extract_error_message(&reason)
                );
            }
            _ => {
                println_err!("Invalid data has been received");
            }
        };

        trace!("execute <<");
        Ok(())
    }
}

pub mod sign_multi_command {
    use super::*;
    use indy_vdr::common::error::VdrErrorKind;

    command!(CommandMetadata::build(
        "sign-multi",
        "Add multi signature by current DID to transaction."
    )
    .add_optional_param(
        "txn",
        "Transaction to sign. Skip to use a transaction stored into CLI context."
    )
    .add_example(r#"ledger sign-multi txn={"reqId":123456789,"type":"100"}"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;

        let param_txn = get_opt_str_param("txn", params).map_err(error_err!())?;

        let mut txn = get_transaction_to_use!(ctx, param_txn);

        match Ledger::multi_sign_request(&store, &submitter_did, &mut txn) {
            Ok(_) => {
                println_succ!("Transaction has been signed:");
                println_succ!("{:?}", txn.req_json.to_string());
                set_transaction(ctx, Some(txn.req_json.to_string()));
            }
            Err(err) => match err {
                CliError::VdrError(ref vdr_err) => match vdr_err.kind() {
                    VdrErrorKind::Unexpected => {
                        println_err!("Signer DID: \"{}\" not found", submitter_did);
                    }
                    _ => {
                        println_err!("{}", err.message(None));
                    }
                },
                _ => {
                    println_err!("{}", err.message(None));
                }
            },
        };

        trace!("execute <<");
        Ok(())
    }
}

pub mod auth_rule_command {
    use super::*;
    use indy_vdr::ledger::constants::txn_name_to_code;

    command!(CommandMetadata::build("auth-rule", "Send AUTH_RULE request to change authentication rules for a ledger transaction.")
                .add_required_param("txn_type", "Ledger transaction alias or associated value")
                .add_required_param("action", "Type of an action. One of: ADD, EDIT")
                .add_required_param("field", "Transaction field")
                .add_optional_param("old_value", "Old value of field, which can be changed to a new_value (mandatory for EDIT action)")
                .add_optional_param("new_value", "New value that can be used to fill the field")
                .add_required_param("constraint", r#"Set of constraints required for execution of an action
         {
             constraint_id - type of a constraint. Can be either "ROLE" to specify final constraint or  "AND"/"OR" to combine constraints, or "FORBIDDEN" to forbid action.
             role - (optional) role associated value {TRUSTEE: 0, STEWARD: 2, TRUST_ANCHOR: 101, ENDORSER: 101, NETWORK_MONITOR: 201, ANY: *}.
             sig_count - the number of signatures required to execution action.
             need_to_be_owner - (optional) if user must be an owner of transaction (false by default).
             off_ledger_signature - (optional) allow signature of unknow for ledger did (false by default).
             metadata - (optional) additional parameters of the constraint.
         }
         can be combined by
         {
             constraint_id: <"AND" or "OR">
             auth_constraints: [<constraint_1>, <constraint_2>]
         }
                "#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false,"off_ledger_signature":true}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=EDIT field=role old_value=101 new_value=0 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"constraint_id":"FORBIDDEN"}""#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let txn_type = get_str_param("txn_type", params).map_err(error_err!())?;
        let action = get_str_param("action", params).map_err(error_err!())?;
        let field = get_str_param("field", params).map_err(error_err!())?;
        let old_value = get_opt_str_param("old_value", params).map_err(error_err!())?;
        let new_value = get_opt_str_param("new_value", params).map_err(error_err!())?;
        let constraint = get_str_param("constraint", params).map_err(error_err!())?;

        let txn_type = txn_name_to_code(txn_type)
            .ok_or_else(|| println_err!("Unsupported ledger transaction."))?;

        let mut request = Ledger::build_auth_rule_request(
            pool.as_deref(),
            &submitter_did,
            &txn_type,
            &action.to_uppercase(),
            field,
            old_value,
            new_value,
            constraint,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        if let Some(result) = response.result.as_mut() {
            result["txn"]["data"]["auth_type"] = get_txn_title(&result["txn"]["data"]["auth_type"]);
            result["txn"]["data"]["constraint"] = JsonValue::String(
                ::serde_json::to_string_pretty(&result["txn"]["data"]["constraint"]).unwrap(),
            );
        }

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Auth Rule request has been sent to Ledger.",
                None,
                &[
                    ("auth_type", "Txn Type"),
                    ("auth_action", "Action"),
                    ("field", "Field"),
                    ("old_value", "Old Value"),
                    ("new_value", "New Value"),
                    ("constraint", "Constraint"),
                ],
                false,
            )
        })?;

        trace!("execute << ");
        Ok(())
    }
}

pub mod auth_rules_command {
    use super::*;

    command!(CommandMetadata::build("auth-rules", "Send AUTH_RULES request to change authentication rules for multiple ledger transactions.")
                .add_main_param("rules", r#"A list of auth rules: [{"auth_type", "auth_action", "field", "old_value", "new_value"},{...}]"#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger auth-rules [{"auth_type":"1","auth_action":"ADD","field":"role","new_value":"101","constraint":{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}}]"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let rules = get_str_param("rules", params).map_err(error_err!())?;

        let mut request = Ledger::build_auth_rules_request(pool.as_deref(), &submitter_did, &rules)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        let result = handle_transaction_response(response)?;
        println!("result {:?}", result);

        let rules: AuthRulesData = serde_json::from_value(result["txn"]["data"]["rules"].clone())
            .map_err(|_| println_err!("Wrong data has been received"))?;
        print_auth_rules(rules);

        trace!("execute << ");
        Ok(())
    }
}

pub type AuthRulesData = Vec<AuthRuleData>;

pub mod get_auth_rule_command {
    use super::*;

    command!(CommandMetadata::build("get-auth-rule", r#"Send GET_AUTH_RULE request to get authentication rules for ledger transactions.
        Note: Either none or all parameters must be specified (`old_value` can be skipped for `ADD` action)."#)
                .add_required_param("txn_type", "Ledger transaction alias or associated value.")
                .add_required_param("action", "Type of action for. One of: ADD, EDIT")
                .add_required_param("field", "Transaction field")
                .add_optional_param("old_value", "Old value of field, which can be changed to a new_value (mandatory for EDIT action)")
                .add_required_param("new_value", "New value that can be used to fill the field")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger get-auth-rule txn_type=NYM action=ADD field=role new_value=101"#)
                .add_example(r#"ledger get-auth-rule txn_type=NYM action=EDIT field=role old_value=101 new_value=0"#)
                .add_example(r#"ledger get-auth-rule"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let auth_type = get_opt_str_param("txn_type", params).map_err(error_err!())?;
        let auth_action = get_opt_str_param("action", params).map_err(error_err!())?;
        let field = get_opt_str_param("field", params).map_err(error_err!())?;
        let old_value = get_opt_str_param("old_value", params).map_err(error_err!())?;
        let new_value = get_opt_str_param("new_value", params).map_err(error_err!())?;

        let request = Ledger::build_get_auth_rule_request(
            pool.as_deref(),
            submitter_did.as_ref(),
            auth_type,
            auth_action,
            field,
            old_value,
            new_value,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        let result = handle_transaction_response(response)?;

        let rules: AuthRulesData = serde_json::from_value(result["data"].clone())
            .map_err(|_| println_err!("Wrong data has been received"))?;

        print_auth_rules(rules);

        trace!("execute << ");
        Ok(())
    }
}

pub mod save_transaction_command {
    use super::*;

    command!(CommandMetadata::build(
        "save-transaction",
        "Save transaction from CLI context into a file."
    )
    .add_required_param("file", "The path to file.")
    .add_example(r#"ledger save-transaction /home/transaction.txt"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let file = get_str_param("file", params).map_err(error_err!())?;

        let transaction = ensure_set_transaction(ctx)?;

        println!("Transaction: {:?}.", transaction);
        println!("Would you like to save it? (y/n)");

        let save_transaction = crate::command_executor::wait_for_user_reply(ctx);

        if !save_transaction {
            println!("The transaction has not been saved.");
            return Ok(());
        }

        write_file(file, &transaction)
            .map_err(|err| println_err!("Cannot store transaction into the file: {:?}", err))?;

        println_succ!("The transaction has been saved.");

        trace!("execute <<");
        Ok(())
    }
}

pub mod load_transaction_command {
    use super::*;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Request {
        pub req_id: u64,
        pub identifier: String,
        pub operation: JsonValue,
    }

    command!(CommandMetadata::build(
        "load-transaction",
        "Read transaction from a file and store it into CLI context."
    )
    .add_required_param("file", "The path to file containing a transaction to load.")
    .add_example(r#"ledger load-transaction /home/transaction.txt"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let file = get_str_param("file", params).map_err(error_err!())?;

        let transaction = read_file(file).map_err(|err| println_err!("{}", err))?;

        serde_json::from_str::<Request>(&transaction)
            .map_err(|err| println_err!("File contains invalid transaction: {:?}", err))?;

        println!("Transaction has been loaded: {}", transaction);

        set_transaction(ctx, Some(transaction));

        trace!("execute <<");
        Ok(())
    }
}

pub mod taa_command {
    use super::*;

    command!(CommandMetadata::build("txn-author-agreement", r#"Send Transaction Author Agreement to the ledger."#)
                .add_optional_param("text", r#"The content of a new agreement.
                         Mandatory in case of adding a new TAA. An existing TAA text can not be changed.
                         for Indy Node version <= 1.12.0:
                             Use empty string to reset TAA on the ledger
                         for Indy Node version > 1.12.0
                             Should be omitted in case of updating an existing TAA (setting `retirement-timestamp`)
                "#)
                .add_optional_param("file", "The path to file containing a content of agreement to send (an alternative to the `text` parameter)")
                .add_required_param("version", "The version of a new agreement")
                .add_optional_param("ratification-timestamp",r#"The date (timestamp) of TAA ratification by network government
                                 for Indy Node version <= 1.12.0:
                                    Must be omitted
                                 for Indy Node version > 1.12.0:
                                    Must be specified in case of adding a new TAA
                                    Can be omitted in case of updating an existing TAA
                "#)
                .add_optional_param("retirement-timestamp", r#"The date (timestamp) of TAA retirement.
                                for Indy Node version <= 1.12.0:
                                    Must be omitted
                                for Indy Node version > 1.12.0:
                                    Must be omitted in case of adding a new (latest) TAA.
                                    Should be used for updating (deactivating) non-latest TAA on the ledger.
                "#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger txn-author-agreement text=\"Indy transaction agreement\" version=1")
                .add_example("ledger txn-author-agreement text= version=1")
                .add_example("ledger txn-author-agreement file=/home/agreement_content.txt version=1")
                .add_example("ledger txn-author-agreement text=\"Indy transaction agreement\" version=1 send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let text = get_opt_empty_str_param("text", params).map_err(error_err!())?;
        let file = get_opt_str_param("file", params).map_err(error_err!())?;
        let version = get_str_param("version", params).map_err(error_err!())?;
        let ratification_ts =
            get_opt_number_param::<u64>("ratification-timestamp", params).map_err(error_err!())?;
        let retirement_ts =
            get_opt_number_param::<u64>("retirement-timestamp", params).map_err(error_err!())?;

        let text: Option<String> = match (text, file) {
            (Some(text_), None) => Some(text_.to_string()),
            (None, Some(file_)) => Some(read_file(file_).map_err(|err| println_err!("{}", err))?),
            (Some(_), Some(_)) => {
                println_err!("Only one of the parameters `text` and `file` can be specified");
                return Err(());
            }
            (None, None) => None,
        };

        let mut request = Ledger::build_txn_author_agreement_request(
            pool.as_deref(),
            &submitter_did,
            text.as_ref().map(String::as_str),
            &version,
            ratification_ts,
            retirement_ts,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            // TODO support multiply active TAA on the ledger IS-1441
            if let Some(text) = text {
                print_transaction_response(
                    result,
                    "Transaction Author Agreement has been sent to Ledger.",
                    None,
                    &[
                        ("text", "Text"),
                        ("version", "Version"),
                        ("ratification_ts", "Ratification Time"),
                        ("retirement_ts", "Retirement Time"),
                    ],
                    true,
                );
                crate::commands::pool::accept_transaction_author_agreement(ctx, &text, &version);
            }
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod aml_command {
    use super::*;

    command!(CommandMetadata::build("txn-acceptance-mechanisms", r#"Send TAA Acceptance Mechanisms to the ledger."#)
                .add_optional_param("aml", "The set of new acceptance mechanisms.")
                .add_optional_param("file", "The path to file containing a set of acceptance mechanisms to send (an alternative to the text parameter).")
                .add_required_param("version", "The version of a new set of acceptance mechanisms.")
                .add_optional_param("context", "Common context information about acceptance mechanisms (may be a URL to external resource).")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1")
                .add_example("ledger txn-acceptance-mechanisms file=/home/mechanism.txt version=1")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1 context=\"some context\"")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1 send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let aml = get_opt_str_param("aml", params).map_err(error_err!())?;
        let file = get_opt_str_param("file", params).map_err(error_err!())?;
        let version = get_str_param("version", params).map_err(error_err!())?;
        let context = get_opt_str_param("context", params).map_err(error_err!())?;

        let aml = match (aml, file) {
            (Some(aml_), None) => aml_.to_string(),
            (None, Some(file_)) => read_file(file_).map_err(|err| println_err!("{}", err))?,
            (Some(_), Some(_)) => {
                println_err!("Only one of the parameters `aml` and `file` can be specified");
                return Err(());
            }
            (None, None) => {
                println_err!("Either `aml` or `file` parameter must be specified");
                return Err(());
            }
        };

        let mut request = Ledger::build_acceptance_mechanisms_request(
            pool.as_deref(),
            &submitter_did,
            &aml,
            &version,
            context,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Acceptance Mechanisms have been sent to Ledger.",
                None,
                &[
                    ("aml", "Text"),
                    ("version", "Version"),
                    ("amlContext", "Context"),
                ],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod taa_disable_all_command {
    use super::*;

    command!(CommandMetadata::build("disable-all-txn-author-agreements", r#"Disable All Transaction Author Agreements on the ledger"#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger disable-all-txn-author-agreements")
                .add_example("ledger disable-all-txn-author-agreements send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, _) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let mut request = Ledger::build_disable_all_txn_author_agreements_request(
            pool.as_deref(),
            &submitter_did,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) = send_write_request!(
            ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        handle_transaction_response(response).map(|_| {
            set_transaction_author_info(ctx, None);
            println_succ!("All Transaction Author Agreements on the Ledger have been disabled");
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod endorse_transaction_command {
    use super::*;

    command!(CommandMetadata::build(
        "endorse",
        "Endorse transaction to the ledger preserving an original author."
    )
    .add_optional_param(
        "txn",
        "Transaction to endorse. Skip to use a transaction stored into CLI context."
    )
    .add_example(r#"ledger endorse txn={"reqId":123456789,"type":"100"}"#)
    .add_example(r#"ledger endorse"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let (store, wallet_name) = ensure_opened_wallet(&ctx)?;
        let submitter_did = ensure_active_did(&ctx)?;

        let param_txn = get_opt_str_param("txn", params).map_err(error_err!())?;

        let mut request = get_transaction_to_use!(ctx, param_txn);

        Ledger::multi_sign_request(&store, &submitter_did, &mut request)
            .map_err(|err| println_err!("{}", err.message(Some(&wallet_name))))?;

        let (_, response) = send_request!(&ctx, params, &request, None, Some(&submitter_did), true);

        handle_transaction_response(response)
            .and_then(|result| parse_transaction_response(result))
            .map(|(metadata_headers, metadata, data)| {
                println_succ!("Transaction has been sent to Ledger.");

                println_succ!("Metadata:");
                print_table(&metadata, &metadata_headers);

                println_succ!("Data:");
                print_table(&json!({ "data": data }), &[("data", "Data")]);
            })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_acceptance_mechanisms_command {
    use super::*;

    command!(CommandMetadata::build("get-acceptance-mechanisms", r#"Get a list of acceptance mechanisms set on the ledger"#)
                .add_optional_param("timestamp","The time (as timestamp) to get an active acceptance mechanisms. Skip to get the latest one")
                .add_optional_param("version","The version of acceptance mechanisms")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-acceptance-mechanisms")
                .add_example("ledger get-acceptance-mechanisms timestamp=1576674598")
                .add_example("ledger get-acceptance-mechanisms version=1.0")
                .add_example("ledger get-acceptance-mechanisms send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = get_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let timestamp = get_opt_number_param::<u64>("timestamp", params).map_err(error_err!())?;
        let version = get_opt_str_param("version", params).map_err(error_err!())?;

        let request = Ledger::build_get_acceptance_mechanisms_request(
            pool.as_deref(),
            submitter_did.as_ref(),
            timestamp,
            version,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request, submitter_did.as_ref());

        match handle_transaction_response(response) {
            Ok(result) => {
                let aml = result["data"]["aml"]
                    .as_object()
                    .ok_or_else(|| println_err!("Wrong data has been received"))?;

                let aml = aml
                    .iter()
                    .map(|(key, value)| {
                        json!({
                            "label": key,
                            "description": value
                        })
                    })
                    .collect::<Vec<JsonValue>>();

                if !aml.is_empty() {
                    println!("Following Acceptance Mechanisms are set on the Ledger");
                }

                print_list_table(
                    &aml,
                    &[("label", "Label"), ("description", "Description")],
                    "There are no acceptance mechanisms",
                );

                println!(
                    "Version: {}",
                    result["data"]["version"].as_str().unwrap_or_default()
                );

                if let Some(context) = result["data"]["amlContext"].as_str() {
                    println!("Context: {}", context);
                }
                println!();
            }
            Err(_) => {}
        }

        trace!("execute <<");
        Ok(())
    }
}

pub mod ledgers_freeze_command {
    use super::*;

    command!(
        CommandMetadata::build("ledgers-freeze", r#"Freeze ledgers"#)
            .add_required_param("ledgers_ids", "List of ledgers IDs for freezing.")
            .add_example("ledger ledgers-freeze ledgers_ids=1,2,3")
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);
        let ledgers_ids = get_number_tuple_array_param("ledgers_ids", params);
        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let (store, _) = ensure_opened_wallet(&ctx)?;

        let mut request =
            Ledger::build_ledgers_freeze_request(pool.as_deref(), &submitter_did, ledgers_ids?)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_write_request!(
            &ctx,
            params,
            &mut request,
            &store,
            &wallet_name,
            &submitter_did
        );

        let result = handle_transaction_response(response)?;

        println_succ!("result {:?}", result);

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_frozen_ledgers_command {
    use super::*;

    command!(
        CommandMetadata::build("get-frozen-ledgers", r#"Get a list of frozen ledgers"#)
            .add_example("ledger get-frozen-ledgers")
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = ensure_active_did(&ctx)?;
        let pool = get_connected_pool(&ctx);

        let request = Ledger::build_get_frozen_ledgers_request(pool.as_deref(), &submitter_did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request, Some(&submitter_did));
        let handle_response = handle_transaction_response(response)?;

        // Flattering ap into vector
        let handle_response = handle_response
            .as_object()
            .expect("top level object is not a map");

        let mut result = Vec::new();
        for (response_ledger_key, response_ledger_value) in handle_response {
            let mut ledger_info = response_ledger_value
                .as_object()
                .expect("inner object is not a map")
                .clone();

            let ledger_id = serde_json::to_value(&response_ledger_key)
                .map_err(|_| println_err!("Invalid format of Outputs: Ledger ID is incorrect."))?;
            ledger_info.insert("ledger_id".to_owned(), ledger_id);

            result
                .push(serde_json::to_value(&ledger_info).map_err(|_| {
                    println_err!("Invalid format of Outputs: result is incorrect.")
                })?);
        }

        print_frozen_ledgers(result)?;
        trace!("execute <<");
        Ok(())
    }

    fn print_frozen_ledgers(frozen_ledgers: Vec<JsonValue>) -> Result<(), ()> {
        println_succ!("Frozen ledgers has been received.");
        print_list_table(
            &frozen_ledgers,
            &[
                ("ledger_id", "Ledger id"),
                ("ledger", "Ledger root hash"),
                ("state", "State root hash"),
                ("seq_no", "Last sequance number"),
            ],
            "No frozen ledgers found.",
        );

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
pub struct AuthRuleData {
    pub auth_type: String,
    pub auth_action: String,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub constraint: JsonValue,
}

fn print_auth_rules(rules: AuthRulesData) {
    let constraints = rules
        .into_iter()
        .map(|rule| {
            let auth_type = get_txn_title(&JsonValue::String(rule.auth_type.clone()));
            let action = rule.auth_action;
            let field = rule.field;
            let old_value = if action == "ADD" {
                None
            } else {
                rule.old_value
            };
            let new_value = rule.new_value;

            json!({
                "auth_type": auth_type,
                "auth_action": action,
                "field": field,
                "old_value": old_value,
                "new_value": new_value,
                "constraint": ::serde_json::to_string_pretty(&rule.constraint).unwrap(),
            })
        })
        .collect::<Vec<JsonValue>>();

    print_list_table(
        &constraints,
        &[
            ("auth_type", "Type"),
            ("auth_action", "Action"),
            ("field", "Field"),
            ("old_value", "Old Value"),
            ("new_value", "New Value"),
            ("constraint", "Constraint"),
        ],
        "There are no rules set",
    );
}

fn sign_and_submit_action(
    store: &AnyStore,
    pool: &LocalPool,
    submitter_did: &DidValue,
    request: &mut PreparedRequest,
    nodes: Option<Vec<&str>>,
    timeout: Option<i64>,
) -> CliResult<String> {
    let nodes = match nodes {
        Some(n) => Some(json!(n).to_string()),
        None => None,
    };

    Ledger::sign_request(store, submitter_did, request)?;
    let replies =
        Ledger::submit_action(pool, &request, nodes.as_ref().map(String::as_ref), timeout)?;

    let replies: HashMap<String, String> = replies
        .into_iter()
        .map(|(node, reply)| (node, reply.to_string()))
        .collect();

    Ok(json!(replies).to_string())
}

pub fn set_author_agreement(ctx: &CommandContext, request: &mut PreparedRequest) -> Result<(), ()> {
    let pool = get_connected_pool(&ctx);

    if let Some((text, version, acc_mech_type, time_of_acceptance)) =
        get_transaction_author_info(&ctx)
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

fn parse_transaction_response(
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

fn print_transaction_response(
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
            println_err!(
                "Transaction has been rejected: {}",
                extract_error_message(&reason)
            );
            Err(())
        }
        _ => {
            println_err!("Invalid data has been received");
            Err(())
        }
    }
}

fn get_role_title(role: &JsonValue) -> JsonValue {
    JsonValue::String(
        match role.as_str() {
            Some("0") => "TRUSTEE",
            Some("2") => "STEWARD",
            Some("101") => "ENDORSER",
            Some("201") => "NETWORK_MONITOR",
            _ => "-",
        }
        .to_string(),
    )
}

fn get_txn_title(role: &JsonValue) -> JsonValue {
    JsonValue::String(
        match role.as_str() {
            Some("0") => "NODE",
            Some("1") => "NYM",
            Some("3") => "GET_TXN",
            Some("4") => "TXN_AUTHR_AGRMT",
            Some("5") => "TXN_AUTHR_AGRMT_AML",
            Some("6") => "GET_TXN_AUTHR_AGRMT",
            Some("7") => "GET_TXN_AUTHR_AGRMT_AML",
            Some("9") => "LEDGERS_FREEZE",
            Some("10") => "GET_FROZEN_LEDGERS",
            Some("100") => "ATTRIB",
            Some("101") => "SCHEMA",
            Some("104") => "GET_ATTR",
            Some("105") => "GET_NYM",
            Some("107") => "GET_SCHEMA",
            Some("108") => "GET_CRED_DEF",
            Some("102") => "CRED_DEF",
            Some("109") => "POOL_UPGRADE",
            Some("111") => "POOL_CONFIG",
            Some("113") => "REVOC_REG_DEF",
            Some("114") => "REVOC_REG_ENTRY",
            Some("115") => "GET_REVOC_REG_DEF",
            Some("116") => "GET_REVOC_REG",
            Some("117") => "GET_REVOC_REG_DELTA",
            Some("118") => "POOL_RESTART",
            Some("119") => "GET_VALIDATOR_INFO",
            Some("120") => "AUTH_RULE",
            Some("121") => "GET_AUTH_RULE",
            Some("122") => "AUTH_RULES",
            Some(val) => val,
            _ => "-",
        }
        .to_string(),
    )
}

fn timestamp_to_datetime(_time: i64) -> String {
    NaiveDateTime::from_timestamp_opt(_time, 0)
        .map(|datetime| datetime.to_string())
        .unwrap_or_default()
}

pub fn get_active_transaction_author_agreement(
    pool: &LocalPool,
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

#[derive(Deserialize, Debug)]
pub struct ReplyResult<T> {
    pub data: T,
    #[serde(rename = "seqNo")]
    pub seq_no: u64,
    pub identifier: String,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::did::tests::{
        new_did, use_did, DID_MY1, DID_MY3, DID_TRUSTEE, SEED_MY3, SEED_TRUSTEE, VERKEY_MY1,
        VERKEY_MY3,
    };
    use crate::commands::pool::tests::disconnect_and_delete_pool;
    use crate::commands::wallet::tests::{
        close_and_delete_wallet, close_wallet, create_and_open_wallet, open_wallet,
    };
    use crate::tools::did::Did;
    use crate::tools::ledger::Ledger;
    use std::ops::Deref;

    const TRANSACTION: &str = r#"{"reqId":1,"identifier":"V4SGRU86Z58d6TV7PBUe6f","operation":{"type":"105","dest":"V4SGRU86Z58d6TV7PBUe6f"},"protocolVersion":2}"#;

    pub const ATTRIB_RAW_DATA: &str = r#"{"endpoint":{"ha":"127.0.0.1:5555"}}"#;
    pub const ATTRIB_HASH_DATA: &str =
        r#"83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3"#;
    pub const ATTRIB_ENC_DATA: &str = r#"aa3f41f619aa7e5e6b6d0d"#;

    pub const CRED_DEF_DATA: &str =
        r#"{"n":"1","s":"1","rms":"1","r":{"age":"1","name":"1"},"rctxt":"1","z":"1"}"#;

    mod nym {
        use super::*;

        #[test]
        pub fn nym_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (did, verkey) = create_new_did(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("verkey", verkey);
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_nym_added(&ctx, &did).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_for_role() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (did, verkey) = create_new_did(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("verkey", verkey);
                params.insert("role", "TRUSTEE".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_nym_added(&ctx, &did).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_for_wrong_role() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);

            let (did, verkey) = create_new_did(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("verkey", verkey);
                params.insert("role", "ROLE".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("verkey", VERKEY_MY1.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_for_no_opened_wallet() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);

            close_and_delete_wallet(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("verkey", VERKEY_MY1.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn nym_works_for_no_connected_pool() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);

            disconnect_and_delete_pool(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("verkey", VERKEY_MY1.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            close_and_delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn nym_works_for_unknown_submitter() {
            let ctx = setup_with_wallet_and_pool();

            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY3.to_string());
                params.insert("verkey", VERKEY_MY3.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (did, verkey) = create_new_did(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("verkey", verkey);
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_nym_added(&ctx, &did).is_err());
            assert!(get_transaction(&ctx).is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_without_signing() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (did, verkey) = create_new_did(&ctx);
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("verkey", verkey);
                params.insert("sign", "false".to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let transaction = get_transaction(&ctx).unwrap();
            let transaction: JsonValue = serde_json::from_str(&transaction).unwrap();
            assert!(transaction["signature"].is_null());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn nym_works_for_disconnected_pool_and_specific_protocol_version() {
            let ctx = setup_with_wallet();
            use_trustee(&ctx);
            let (did, _) = create_new_did(&ctx);
            // Set Custom Pool protocol version
            {
                let cmd = pool::set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert("protocol-version", "1".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            // Build NYM request
            {
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            // Reset Custom Pool protocol version
            {
                let cmd = pool::set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "protocol-version",
                    DEFAULT_POOL_PROTOCOL_VERSION.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
        }
    }

    mod get_nym {
        use super::*;

        #[test]
        pub fn get_nym_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_nym_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = get_nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_nym_works_for_unknown_did() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY3.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod attrib {
        use super::*;

        #[test]
        pub fn attrib_works_for_raw_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_hash_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, None, Some(ATTRIB_HASH_DATA), None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_enc_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, None, None, Some(ATTRIB_ENC_DATA)).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_missed_attribute() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_unknown_did() {
            let ctx = setup_with_wallet_and_pool();

            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY3.to_string());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_invalid_endpoint_format() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                params.insert("raw", r#"127.0.0.1:5555"#.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_raw_value_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_err());
            assert!(get_transaction(&ctx).is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_without_signing() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("sign", "false".to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let transaction = get_transaction(&ctx).unwrap();
            let transaction: JsonValue = serde_json::from_str(&transaction).unwrap();
            assert!(transaction["signature"].is_null());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_endorser() {
            let ctx = setup_with_wallet_and_pool();
            let (endorser_did, _) = use_new_identity(&ctx);

            // Publish new NYM without any role
            let (did, verkey) = create_new_did(&ctx);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("endorser", endorser_did.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            use_did(&ctx, &endorser_did);
            {
                let cmd = endorse_transaction_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod get_attrib {
        use super::*;

        #[test]
        pub fn get_attrib_works_for_raw_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", "endpoint".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_hash_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, None, Some(ATTRIB_HASH_DATA), None).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_enc_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, None, None, Some(ATTRIB_ENC_DATA)).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());

            // to reset active did
            close_wallet(&ctx);
            open_wallet(&ctx);

            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", "endpoint".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod schema {
        use super::*;

        #[test]
        pub fn schema_works() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_schema_added(&ctx, &did).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_for_missed_required_params() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_unknown_submitter() {
            let ctx = setup_with_wallet_and_pool();
            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_schema_added(&ctx, &did).is_err());
            assert!(get_transaction(&ctx).is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_without_signing() {
            let ctx = setup_with_wallet_and_pool();
            use_new_identity(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                params.insert("sign", "false".to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let transaction = get_transaction(&ctx).unwrap();
            let transaction: JsonValue = serde_json::from_str(&transaction).unwrap();
            assert!(transaction["signature"].is_null());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_for_endorser() {
            let ctx = setup_with_wallet_and_pool();
            let (endorser_did, _) = use_new_identity(&ctx);

            // Publish new NYM without any role
            let (did, verkey) = create_new_did(&ctx);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                params.insert("endorser", endorser_did.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            use_did(&ctx, &endorser_did);
            {
                let cmd = endorse_transaction_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_schema_added(&ctx, &did).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod get_validator_info {
        use super::*;

        #[test]
        pub fn get_validator_info_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_validator_info_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_validator_info_works_for_nodes() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_validator_info_command::new();
                let mut params = CommandParams::new();
                params.insert("nodes", "Node1,Node2".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_validator_info_works_for_unknown_node() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_validator_info_command::new();
                let mut params = CommandParams::new();
                params.insert("nodes", "Unknown Node".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_validator_info_works_for_timeout() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_validator_info_command::new();
                let mut params = CommandParams::new();
                params.insert("nodes", "Node1,Node2".to_string());
                params.insert("timeout", "10".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod get_schema {
        use super::*;

        #[test]
        pub fn get_schema_works() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_schema_added(&ctx, &did).is_ok());
            {
                let cmd = get_schema_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did);
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_for_unknown_schema() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_schema_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                params.insert("name", "unknown_schema_name".to_string());
                params.insert("version", "1.0".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test] // TODO: CHECK
        pub fn schema_works_for_unknown_submitter() {
            let ctx = setup_with_wallet_and_pool();
            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = get_schema_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY3.to_string());
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn schema_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            {
                let cmd = schema_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                params.insert("attr_names", "name,age".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_schema_added(&ctx, &did).is_ok());

            // to reset active did
            close_wallet(&ctx);
            open_wallet(&ctx);

            {
                let cmd = get_schema_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did);
                params.insert("name", "gvt".to_string());
                params.insert("version", "1.0".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod cred_def {
        use super::*;

        #[test]
        pub fn cred_def_works() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            let schema_id = send_schema(&ctx, &did);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id.clone());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_cred_def_added(&ctx, &did, &schema_id).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn cred_def_works_for_missed_required_params() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", "1".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn cred_def_works_for_unknown_submitter() {
            let ctx = setup_with_wallet_and_pool();
            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", "1".to_string());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn cred_def_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", "1".to_string());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn cred_def_works_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            let schema_id = send_schema(&ctx, &did);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id.clone());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_cred_def_added(&ctx, &did, &schema_id).is_err());
            assert!(get_transaction(&ctx).is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod get_cred_def {
        use super::*;

        #[test]
        pub fn get_cred_def_works() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            let schema_id = send_schema(&ctx, &did);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id.clone());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_cred_def_added(&ctx, &did, &schema_id).is_ok());
            {
                let cmd = get_cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id);
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("origin", did.clone());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_cred_def_works_for_unknown_cred_def() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", "2".to_string());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("origin", DID_MY3.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_cred_def_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_identity(&ctx);
            let schema_id = send_schema(&ctx, &did);
            {
                let cmd = cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id.clone());
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("primary", CRED_DEF_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(_ensure_cred_def_added(&ctx, &did, &schema_id).is_ok());

            // to reset active did
            close_wallet(&ctx);
            open_wallet(&ctx);

            {
                let cmd = get_cred_def_command::new();
                let mut params = CommandParams::new();
                params.insert("schema_id", schema_id);
                params.insert("signature_type", "CL".to_string());
                params.insert("tag", "TAG".to_string());
                params.insert("origin", did.clone());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod node {
        use super::*;

        #[test]
        #[ignore] //TODO: FIXME currently unstable pool behaviour after new non-existing node was added
        pub fn node_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (_did, my_verkey) = create_new_did(&ctx);
            send_nym(&ctx, &_did, &my_verkey, Some("STEWARD"));
            use_did(&ctx, &_did);
            {
                let cmd = node_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "target",
                    "A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y".to_string(),
                );
                params.insert("node_ip", "127.0.0.1".to_string());
                params.insert("node_port", "9710".to_string());
                params.insert("client_ip", "127.0.0.2".to_string());
                params.insert("client_port", "9711".to_string());
                params.insert("alias", "Node5".to_string());
                params.insert("blskey", "2zN3bHM1m4rLz54MJHYSwvqzPchYp8jkHswveCLAEJVcX6Mm1wHQD1SkPYMzUDTZvWvhuE6VNAkK3KxVeEmsanSmvjVkReDeBEMxeDaayjcZjFGPydyey1qxBHmTvAnBKoPydvuTAqx5f7YNNRAdeLmUi99gERUU7TD8KfAa6MpQ9bw".to_string());
                params.insert("blskey_pop", "RPLagxaR5xdimFzwmzYnz4ZhWtYQEj8iR5ZU53T2gitPCyCHQneUn2Huc4oeLd2B2HzkGnjAff4hWTJT6C7qHYB1Mv2wU5iHHGFWkhnTX9WsEAbunJCV2qcaXScKj4tTfvdDKfLiVuU2av6hbsMztirRze7LvYBkRHV3tGwyCptsrP".to_string());
                params.insert("services", "VALIDATOR".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod pool_config {
        use super::*;

        #[test]
        pub fn pool_config_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_config_command::new();
                let mut params = CommandParams::new();
                params.insert("writes", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = pool_config_command::new();
                let mut params = CommandParams::new();
                params.insert("writes", "true".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod pool_restart {
        use super::*;

        #[test]
        pub fn pool_restart_works() {
            let datetime = r#"2020-01-25T12:49:05.258870+00:00"#;

            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_restart_command::new();
                let mut params = CommandParams::new();
                params.insert("action", "start".to_string());
                params.insert("datetime", datetime.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn pool_restart_works_for_nodes() {
            let datetime = r#"2020-01-25T12:49:05.258870+00:00"#;

            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_restart_command::new();
                let mut params = CommandParams::new();
                params.insert("action", "start".to_string());
                params.insert("datetime", datetime.to_string());
                params.insert("nodes", "Node1,Node2".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn pool_restart_works_for_timeout() {
            let datetime = r#"2020-01-25T12:49:05.258870+00:00"#;

            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_restart_command::new();
                let mut params = CommandParams::new();
                params.insert("action", "start".to_string());
                params.insert("datetime", datetime.to_string());
                params.insert("nodes", "Node1,Node2".to_string());
                params.insert("timeout", "10".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod pool_upgrade {
        use super::*;

        #[test]
        #[ignore]
        pub fn pool_upgrade_works() {
            let schedule = r#"{"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00",
                                    "8ECVSk179mjsjKRLWiQtssMLgp6EPhWXtaYyStWPSGAb":"2020-01-25T13:49:05.258870+00:00",
                                    "DKVxG2fXXTU8yT5N7hGEbXB3dfdAnYv1JczDUHpmDxya":"2020-01-25T14:49:05.258870+00:00",
                                    "4PS3EDQ3dW1tci1Bp6543CfuuebjFrg36kLAUcskGfaA":"2020-01-25T15:49:05.258870+00:00"}"#;

            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_upgrade_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "upgrade-indy-cli".to_string());
                params.insert("version", "2.0.0".to_string());
                params.insert("action", "start".to_string());
                params.insert(
                    "sha256",
                    "f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398".to_string(),
                );
                params.insert("schedule", schedule.to_string());
                params.insert("force", "true".to_string()); // because node_works test added fifth Node
                cmd.execute(&ctx, &params).unwrap();
            }
            // There is no way to read upgrade transaction to be sure about completely write before sending next one.
            // So just sleep agains other places where control read request is available
            ::std::thread::sleep(::std::time::Duration::from_secs(1));
            {
                let cmd = pool_upgrade_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "upgrade-indy-cli".to_string());
                params.insert("version", "2.0.0".to_string());
                params.insert("action", "cancel".to_string());
                params.insert(
                    "sha256",
                    "ac3eb2cc3ac9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod custom {
        use super::*;

        pub const TXN_FOR_SIGN: &str = r#"{
                                                    "reqId":1513241300414292814,
                                                    "identifier":"V4SGRU86Z58d6TV7PBUe6f",
                                                    "operation":{
                                                        "type":"1",
                                                        "dest":"E1XWGvsrVp5ZDif2uDdTAM",
                                                        "verkey":"86F43kmApX7Da5Rcba1vCbYmc7bbauEksGxPKy8PkZyb"
                                                    },
                                                    "protocolVersion":2
                                                  }"#;

        #[test]
        pub fn custom_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn custom_works_for_sign() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert("sign", "true".to_string());
                params.insert("txn", TXN_FOR_SIGN.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn custom_works_for_missed_txn_field() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = custom_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn custom_works_for_invalid_transaction_format() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "txn",
                    format!(
                        r#"
                                                    "reqId":1513241300414292814,
                                                    "identifier":"{}",
                                                    "protocolVersion":2
                                                  "#,
                        DID_TRUSTEE
                    ),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn custom_works_for_no_opened_pool() {
            let ctx = setup();

            create_and_open_wallet(&ctx);

            use_trustee(&ctx);
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            close_and_delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn custom_works_for_sign_without_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert("sign", "true".to_string());
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn custom_works_for_unknown_submitter_did() {
            let ctx = setup_with_wallet_and_pool();

            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = custom_command::new();
                let mut params = CommandParams::new();
                params.insert("sign", "true".to_string());
                params.insert("txn", TXN_FOR_SIGN.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod sign_multi {
        use super::*;

        #[test]
        pub fn sign_multi_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn sign_multi_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn sign_multi_works_for_invalid_message_format() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", r#"1496822211362017764"#.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod auth_rule {
        use super::*;

        const AUTH_TYPE: &str = "NYM";
        const AUTH_ACTION: &str = "ADD";
        const FIELD: &str = "role";
        const NEW_VALUE: &str = "101";
        const ROLE_CONSTRAINT: &str = r#"{
            "sig_count": 1,
            "metadata": {},
            "role": "0",
            "constraint_id": "ROLE",
            "need_to_be_owner": false
        }"#;

        #[test]
        pub fn auth_rule_works_for_adding_new_trustee() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "ADD".to_string());
                params.insert("field", "role".to_string());
                params.insert("new_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn auth_rule_works_for_demoting_trustee() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "EDIT".to_string());
                params.insert("field", "role".to_string());
                params.insert("old_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_one_constraint() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", FIELD.to_string());
                params.insert("new_value", NEW_VALUE.to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            {
                let cmd = get_auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", FIELD.to_string());
                params.insert("new_value", NEW_VALUE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_get_all() {
            let ctx = setup_with_wallet_and_pool();

            {
                let cmd = get_auth_rule_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_no_constraint() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", "WRONG_FIELD".to_string());
                params.insert("new_value", "WRONG_VALUE".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn auth_rule_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "ADD".to_string());
                params.insert("field", "role".to_string());
                params.insert("new_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(get_transaction(&ctx).is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod save_transaction {
        use super::*;

        #[test]
        pub fn save_transaction_works_for_no_txn_into_context() {
            let ctx = setup();

            let (_, path_str) = _path();
            {
                let cmd = save_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str);
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }
    }

    mod load_transaction {
        use super::*;

        #[test]
        pub fn load_transaction_works() {
            let ctx = setup();

            let (_, path_str) = _path();
            write_file(&path_str, TRANSACTION).unwrap();

            {
                let cmd = load_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str);
                cmd.execute(&ctx, &params).unwrap();
            }

            let context_txn = get_transaction(&ctx).unwrap();

            assert_eq!(TRANSACTION.to_string(), context_txn);

            tear_down();
        }

        #[test]
        pub fn load_transaction_works_for_invalid_transaction() {
            let ctx = setup();

            let (_, path_str) = _path();
            write_file(&path_str, "some invalid transaction").unwrap();

            {
                let cmd = load_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str);
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }

        #[test]
        pub fn load_transaction_works_for_no_file() {
            let ctx = setup();
            {
                let cmd = load_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", "/path/to/file.txt".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test] // IS-1493 save-transaction does not quote JSON output correctly
        pub fn load_save_transaction_works_for_rewriting() {
            let ctx = setup();

            let short_request =
                json!({"reqId": 111, "identifier": DID_TRUSTEE, "operation": {"type": "1"}})
                    .to_string();
            let long_request = json!({"reqId": 111, "identifier": DID_TRUSTEE, "operation": {"type": "1", "data": "some extra data to make it long"}}).to_string();

            // Write long
            let (_, path_str) = _path();
            {
                set_transaction(&ctx, Some(long_request));

                let cmd = save_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str.clone());
                cmd.execute(&ctx, &params).unwrap();
            }

            // Write short
            let (_, path_str) = _path();
            {
                set_transaction(&ctx, Some(short_request));

                let cmd = save_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str.clone());
                cmd.execute(&ctx, &params).unwrap();
            }

            // Load transaction
            {
                let cmd = load_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str.clone());
                cmd.execute(&ctx, &params).unwrap();
            }

            tear_down();
        }
    }

    mod aml {
        use super::*;

        pub const AML: &str = r#"{"Acceptance Mechanism 1": "Description 1", "Acceptance Mechanism 2": "Description 2"}"#;

        pub fn _get_version() -> String {
            Utc::now().timestamp().to_string()
        }

        #[test]
        pub fn acceptance_mechanisms_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = aml_command::new();
                let mut params = CommandParams::new();
                params.insert("aml", AML.to_string());
                params.insert("version", _get_version());
                params.insert("context", "Some Context".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ::std::thread::sleep(::std::time::Duration::from_secs(1));
            {
                let cmd = get_acceptance_mechanisms_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod taa {
        use super::*;

        #[test]
        pub fn taa_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                // Set AML
                let cmd = aml_command::new();
                let mut params = CommandParams::new();
                params.insert("aml", super::aml::AML.to_string());
                params.insert("version", super::aml::_get_version());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Set TAA
                let cmd = taa_command::new();
                let mut params = CommandParams::new();
                params.insert("text", "test taa".to_string());
                params.insert("version", super::aml::_get_version());
                params.insert("ratification-timestamp", "123456789".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Send Nym
                let (did, _) = create_new_did(&ctx);
                ctx.set_taa_acceptance_mechanism("Acceptance Mechanism 1");
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did);
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Disable all TAAs
                let cmd = taa_disable_all_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod frozen_ledgers {
        use super::*;

        #[test]
        pub fn ledgers_freeze() {
            let ctx = setup();

            {
                let cmd = ledgers_freeze_command::new();
                let mut params = CommandParams::new();
                params.insert("ledgers_ids", "0,1,10,237".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }

        #[test]
        pub fn get_frozen_ledgers() {
            let ctx = setup();

            {
                let cmd = get_frozen_ledgers_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }
    }

    fn _path() -> (::std::path::PathBuf, String) {
        let mut path = crate::utils::environment::EnvironmentUtils::indy_home_path();
        path.push("transaction");
        (path.clone(), path.to_str().unwrap().to_string())
    }

    fn create_new_did(ctx: &CommandContext) -> (String, String) {
        let (wallet, _) = get_opened_wallet(ctx).unwrap();
        Did::new(&wallet, None, None, None, None).unwrap()
    }

    fn use_trustee(ctx: &CommandContext) {
        new_did(&ctx, SEED_TRUSTEE);
        use_did(&ctx, DID_TRUSTEE);
    }

    fn use_new_identity(ctx: &CommandContext) -> (String, String) {
        use_trustee(ctx);
        let (did, verkey) = create_new_did(ctx);
        send_nym(ctx, &did, &verkey, Some("ENDORSER"));
        use_did(&ctx, &did);
        (did, verkey)
    }

    pub fn send_schema(ctx: &CommandContext, did: &str) -> String {
        let pool = get_connected_pool(ctx).unwrap();
        let (wallet, _) = get_opened_wallet(ctx).unwrap();
        let did = DidValue(did.to_string());
        let name = "cli_gvt";
        let version = "1.0";
        let attr_names = ["name"];
        let id = SchemaId::new(&did, name, version);
        let schema = Schema::SchemaV1(SchemaV1 {
            id,
            name: name.to_string(),
            version: version.to_string(),
            attr_names: AttributeNames::from(attr_names.as_slice()),
            seq_no: None,
        });
        let mut schema_request =
            Ledger::build_schema_request(Some(pool.deref()), &did, schema).unwrap();
        let schema_response =
            Ledger::sign_and_submit_request(pool.deref(), &wallet, &did, &mut schema_request)
                .unwrap();
        let schema: JsonValue = serde_json::from_str(&schema_response).unwrap();
        let seq_no = schema["result"]["txnMetadata"]["seqNo"].as_i64().unwrap();
        seq_no.to_string()
    }

    pub fn send_nym(ctx: &CommandContext, did: &str, verkey: &str, role: Option<&str>) {
        let cmd = nym_command::new();
        let mut params = CommandParams::new();
        params.insert("did", did.to_string());
        params.insert("verkey", verkey.to_string());
        if let Some(role) = role {
            params.insert("role", role.to_string());
        }
        cmd.execute(&ctx, &params).unwrap();
    }

    fn _ensure_nym_added(ctx: &CommandContext, did: &str) -> Result<(), ()> {
        let pool = get_connected_pool(ctx).unwrap();
        let did = DidValue(did.to_string());
        let request = Ledger::build_get_nym_request(Some(&pool), None, &did).unwrap();
        submit_retry(ctx, &request, |response| {
            serde_json::from_str::<Response<ReplyResult<String>>>(&response).and_then(|response| {
                serde_json::from_str::<JsonValue>(&response.result.unwrap().data)
            })
        })
    }

    fn _ensure_attrib_added(
        ctx: &CommandContext,
        did: &str,
        raw: Option<&str>,
        hash: Option<&str>,
        enc: Option<&str>,
    ) -> Result<(), ()> {
        let pool = get_connected_pool(ctx).unwrap();
        let attr = if raw.is_some() {
            Some("endpoint")
        } else {
            None
        };
        let did = DidValue(did.to_string());
        let request =
            Ledger::build_get_attrib_request(Some(&pool), None, &did, attr, hash, enc).unwrap();
        submit_retry(ctx, &request, |response| {
            serde_json::from_str::<Response<ReplyResult<String>>>(&response)
                .map_err(|_| ())
                .and_then(|response| {
                    let expected_value = if raw.is_some() {
                        raw.unwrap()
                    } else if hash.is_some() {
                        hash.unwrap()
                    } else {
                        enc.unwrap()
                    };
                    if response.result.is_some() && expected_value == response.result.unwrap().data
                    {
                        Ok(())
                    } else {
                        Err(())
                    }
                })
        })
    }

    fn _ensure_schema_added(ctx: &CommandContext, did: &str) -> Result<(), ()> {
        let pool = get_connected_pool(ctx).unwrap();
        let id = SchemaId::new(&DidValue(did.to_string()), "gvt", "1.0");
        let request = Ledger::build_get_schema_request(Some(&pool), None, &id).unwrap();
        submit_retry(ctx, &request, |response| {
            let schema: JsonValue = serde_json::from_str(&response).unwrap();
            schema["result"]["seqNo"].as_i64().ok_or(())
        })
    }

    fn _ensure_cred_def_added(ctx: &CommandContext, did: &str, schema_id: &str) -> Result<(), ()> {
        let pool = get_connected_pool(&ctx).unwrap();
        let schema_id = SchemaId::from_str(schema_id).unwrap();
        let id = CredentialDefinitionId::new(&DidValue(did.to_string()), &schema_id, "CL", "TAG");
        let request = Ledger::build_get_cred_def_request(Some(&pool), None, &id).unwrap();
        submit_retry(ctx, &request, |response| {
            let cred_def: JsonValue = serde_json::from_str(&response).unwrap();
            cred_def["result"]["seqNo"].as_i64().ok_or(())
        })
    }
}
