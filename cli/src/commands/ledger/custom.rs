/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response, ResponseType},
};

use indy_vdr::pool::PreparedRequest;
use serde_json::Value as JsonValue;

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

        let pool = ctx.ensure_connected_pool()?;

        let txn = ParamParser::get_str_param("txn", params)?;
        let sign = ParamParser::get_opt_bool_param("sign", params)?.unwrap_or(false);

        let mut transaction = txn.to_string();

        if txn == "context" {
            let context_txn = ctx.get_context_transaction();

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
                        build a transaction (with passing a `send=false`) to wallet it into CLI context.");
                    return Err(());
                }
            }
        }

        let mut transaction = PreparedRequest::from_request_json(transaction)
            .map_err(|_| println_err!("Invalid formatted transaction provided."))?;

        let response_json = if sign {
            let wallet = ctx.ensure_opened_wallet()?;
            let submitter_did = ctx.ensure_active_did()?;
            Ledger::sign_and_submit_request(&pool, &wallet, &submitter_did, &mut transaction)
                .map_err(|err| println_err!("{}", err.message(Some(&pool.name))))?
        } else {
            Ledger::submit_request(&pool, &transaction)
                .map_err(|err| println_err!("{}", err.message(Some(&pool.name))))?
        };

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
                println_err!("Transaction has been rejected: {}", reason);
            }
            _ => {
                println_err!("Invalid data has been received");
            }
        };

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{
            did::tests::{new_did, use_did, DID_MY3, DID_TRUSTEE, SEED_MY3},
            setup_with_wallet_and_pool, tear_down_with_wallet_and_pool,
            wallet::tests::{close_and_delete_wallet, create_and_open_wallet},
        },
        ledger::tests::{use_trustee, TRANSACTION},
    };

    mod custom {
        use super::*;
        use crate::commands::{setup, tear_down};

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
}
