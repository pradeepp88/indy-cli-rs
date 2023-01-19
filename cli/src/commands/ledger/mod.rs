/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{CommandGroup, CommandGroupMetadata};

#[macro_use]
pub mod common;
pub mod attrib;
pub mod auth_rule;
pub mod constants;
pub mod cred_def;
pub mod custom;
pub mod endorser;
pub mod frozen_ledger;
pub mod node;
pub mod nym;
pub mod pool_config;
pub mod pool_restart;
pub mod pool_upgrade;
pub mod schema;
pub mod sign_multi;
pub mod transaction;
pub mod transaction_author_agreement;
pub mod validator_info;

pub use self::{
    attrib::*, auth_rule::*, common::*, cred_def::*, custom::*, endorser::*, frozen_ledger::*,
    node::*, nym::*, pool_config::*, pool_restart::*, pool_upgrade::*, schema::*, sign_multi::*,
    transaction::*, transaction_author_agreement::*, validator_info::*,
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "ledger",
        "Ledger management commands"
    ));
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        command_executor::{CommandContext, CommandParams},
        commands::{
            did::tests::{new_did, use_did, DID_TRUSTEE, SEED_TRUSTEE},
            submit_retry,
        },
        tools::{
            did::Did,
            ledger::{Ledger, Response},
        },
    };
    use indy_utils::{did::DidValue, Qualifiable};
    use indy_vdr::ledger::{
        identifiers::{CredentialDefinitionId, SchemaId},
        requests::schema::{AttributeNames, Schema, SchemaV1},
    };
    use serde_json::Value as JsonValue;
    use std::ops::Deref;

    #[derive(Deserialize, Debug)]
    struct ReplyResult<T> {
        pub data: T,
    }

    pub const TRANSACTION: &str = r#"{"reqId":1,"identifier":"V4SGRU86Z58d6TV7PBUe6f","operation":{"type":"105","dest":"V4SGRU86Z58d6TV7PBUe6f"},"protocolVersion":2}"#;

    pub const ATTRIB_RAW_DATA: &str = r#"{"endpoint":{"ha":"127.0.0.1:5555"}}"#;
    pub const ATTRIB_HASH_DATA: &str =
        r#"83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3"#;
    pub const ATTRIB_ENC_DATA: &str = r#"aa3f41f619aa7e5e6b6d0d"#;

    pub const CRED_DEF_DATA: &str =
        r#"{"n":"1","s":"1","rms":"1","r":{"age":"1","name":"1"},"rctxt":"1","z":"1"}"#;

    pub fn _path() -> (::std::path::PathBuf, String) {
        let mut path = crate::utils::environment::EnvironmentUtils::indy_home_path();
        path.push("transaction");
        (path.clone(), path.to_str().unwrap().to_string())
    }

    pub fn create_new_did(ctx: &CommandContext) -> (String, String) {
        let (wallet, _) = ctx.get_opened_wallet().unwrap();
        Did::create(&wallet, None, None, None, None).unwrap()
    }

    pub fn use_trustee(ctx: &CommandContext) {
        new_did(&ctx, SEED_TRUSTEE);
        use_did(&ctx, DID_TRUSTEE);
    }

    pub fn use_new_identity(ctx: &CommandContext) -> (String, String) {
        use_trustee(ctx);
        let (did, verkey) = create_new_did(ctx);
        send_nym(ctx, &did, &verkey, Some("ENDORSER"));
        use_did(&ctx, &did);
        (did, verkey)
    }

    pub fn send_schema(ctx: &CommandContext, did: &str) -> String {
        let pool = ctx.get_connected_pool().unwrap();
        let (wallet, _) = ctx.get_opened_wallet().unwrap();
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

    pub fn _ensure_nym_added(ctx: &CommandContext, did: &str) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
        let did = DidValue(did.to_string());
        let request = Ledger::build_get_nym_request(Some(&pool), None, &did).unwrap();
        submit_retry(ctx, &request, |response| {
            serde_json::from_str::<Response<ReplyResult<String>>>(&response).and_then(|response| {
                serde_json::from_str::<JsonValue>(&response.result.unwrap().data)
            })
        })
    }

    pub fn ensure_attrib_added(
        ctx: &CommandContext,
        did: &str,
        raw: Option<&str>,
        hash: Option<&str>,
        enc: Option<&str>,
    ) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
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

    pub fn ensure_schema_added(ctx: &CommandContext, did: &str) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
        let id = SchemaId::new(&DidValue(did.to_string()), "gvt", "1.0");
        let request = Ledger::build_get_schema_request(Some(&pool), None, &id).unwrap();
        submit_retry(ctx, &request, |response| {
            let schema: JsonValue = serde_json::from_str(&response).unwrap();
            schema["result"]["seqNo"].as_i64().ok_or(())
        })
    }

    pub fn ensure_cred_def_added(
        ctx: &CommandContext,
        did: &str,
        schema_id: &str,
    ) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
        let schema_id = SchemaId::from_str(schema_id).unwrap();
        let id = CredentialDefinitionId::new(&DidValue(did.to_string()), &schema_id, "CL", "TAG");
        let request = Ledger::build_get_cred_def_request(Some(&pool), None, &id).unwrap();
        submit_retry(ctx, &request, |response| {
            let cred_def: JsonValue = serde_json::from_str(&response).unwrap();
            cred_def["result"]["seqNo"].as_i64().ok_or(())
        })
    }
}
