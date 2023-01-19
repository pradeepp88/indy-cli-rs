/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response},
};

use serde_json::Value as JsonValue;

use super::common::{
    handle_transaction_response, print_transaction_response, set_author_agreement,
};

pub mod nym_command {
    use super::*;
    use crate::tools::{did::Did, ledger::LedgerHelpers};

    command!(
    CommandMetadata::build("nym", r#"Send NYM transaction to the Ledger."#)
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

        let wallet = ctx.ensure_opened_wallet()?;
        let pool = ctx.get_connected_pool();
        let submitter_did = ctx.ensure_active_did()?;

        let target_did = ParamParser::get_did_param("did", params)?;
        let verkey = ParamParser::get_opt_str_param("verkey", params)?;
        let role = ParamParser::get_opt_empty_str_param("role", params)?;

        if let Some(target_verkey) = verkey {
            let did_info = Did::get(&wallet, &target_did);

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

        let (_, mut response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        if let Some(result) = response.result.as_mut() {
            result["txn"]["data"]["role"] =
                LedgerHelpers::get_role_title(&result["txn"]["data"]["role"]);
            result["role"] = LedgerHelpers::get_role_title(&result["role"]);
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
    use crate::tools::ledger::LedgerHelpers;

    command!(CommandMetadata::build("get-nym", "Get NYM from Ledger.")
                .add_required_param("did","DID of identity presented in Ledger")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-nym did=VsKV7grR1BUE29mG2Fm2kX")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = ctx.get_active_did()?;
        let pool = ctx.get_connected_pool();

        let target_did = ParamParser::get_did_param("did", params)?;

        let request =
            Ledger::build_get_nym_request(pool.as_deref(), submitter_did.as_deref(), &target_did)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response) = send_read_request!(&ctx, params, &request);

        if let Some(result) = response.result.as_mut() {
            let data = serde_json::from_str::<JsonValue>(&result["data"].as_str().unwrap_or(""));
            match data {
                Ok(mut data) => {
                    data["role"] = LedgerHelpers::get_role_title(&data["role"]);
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{
            did::tests::{
                new_did, use_did, DID_MY1, DID_MY3, DID_TRUSTEE, SEED_MY3, VERKEY_MY1, VERKEY_MY3,
            },
            pool::tests::disconnect_and_delete_pool,
            setup_with_wallet, setup_with_wallet_and_pool, submit_retry, tear_down,
            tear_down_with_wallet, tear_down_with_wallet_and_pool,
            wallet::tests::close_and_delete_wallet,
        },
        ledger::tests::{create_new_did, use_trustee, ReplyResult},
        pool::constants::DEFAULT_POOL_PROTOCOL_VERSION,
    };
    use indy_utils::did::DidValue;

    mod nym {
        use super::*;
        use crate::pool::set_protocol_version_command;

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
            assert!(ensure_nym_added(&ctx, &did).is_ok());
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
            assert!(ensure_nym_added(&ctx, &did).is_ok());
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
            assert!(ensure_nym_added(&ctx, &did).is_err());
            assert!(ctx.get_context_transaction().is_some());
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
            let transaction = ctx.get_context_transaction().unwrap();
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
                let cmd = set_protocol_version_command::new();
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
                let cmd = set_protocol_version_command::new();
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

    pub fn ensure_nym_added(ctx: &CommandContext, did: &str) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
        let did = DidValue(did.to_string());
        let request = Ledger::build_get_nym_request(Some(&pool), None, &did).unwrap();
        submit_retry(ctx, &request, |response| {
            serde_json::from_str::<Response<ReplyResult<String>>>(&response).and_then(|response| {
                serde_json::from_str::<JsonValue>(&response.result.unwrap().data)
            })
        })
    }
}
