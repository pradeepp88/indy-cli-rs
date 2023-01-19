/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    utils::file::{read_file, write_file},
};

use serde_json::Value as JsonValue;

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

        let file = ParamParser::get_str_param("file", params)?;

        let transaction = ctx.ensure_context_transaction()?;

        println!("Transaction: {:?}.", transaction);
        println!("Would you like to save it? (y/n)");

        let save_transaction = crate::command_executor::wait_for_user_reply(ctx);

        if !save_transaction {
            println!("The transaction has not been saved.");
            return Ok(());
        }

        write_file(file, &transaction)
            .map_err(|err| println_err!("Cannot wallet transaction into the file: {:?}", err))?;

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
        "Read transaction from a file and wallet it into CLI context."
    )
    .add_required_param("file", "The path to file containing a transaction to load.")
    .add_example(r#"ledger load-transaction /home/transaction.txt"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let file = ParamParser::get_str_param("file", params)?;

        let transaction = read_file(file).map_err(|err| println_err!("{}", err))?;

        serde_json::from_str::<Request>(&transaction)
            .map_err(|err| println_err!("File contains invalid transaction: {:?}", err))?;

        println!("Transaction has been loaded: {}", transaction);

        ctx.set_context_transaction(Some(transaction));

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{did::tests::DID_TRUSTEE, setup, tear_down},
        ledger::tests::TRANSACTION,
    };

    fn path() -> (::std::path::PathBuf, String) {
        let mut path = crate::utils::environment::EnvironmentUtils::indy_home_path();
        path.push("transaction");
        (path.clone(), path.to_str().unwrap().to_string())
    }

    mod save_transaction {
        use super::*;

        #[test]
        pub fn save_transaction_works_for_no_txn_into_context() {
            let ctx = setup();

            let (_, path_str) = path();
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

            let (_, path_str) = path();
            write_file(&path_str, TRANSACTION).unwrap();

            {
                let cmd = load_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str);
                cmd.execute(&ctx, &params).unwrap();
            }

            let context_txn = ctx.get_context_transaction().unwrap();

            assert_eq!(TRANSACTION.to_string(), context_txn);

            tear_down();
        }

        #[test]
        pub fn load_transaction_works_for_invalid_transaction() {
            let ctx = setup();

            let (_, path_str) = path();
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
            let (_, path_str) = path();
            {
                ctx.set_context_transaction(Some(long_request));

                let cmd = save_transaction_command::new();
                let mut params = CommandParams::new();
                params.insert("file", path_str.clone());
                cmd.execute(&ctx, &params).unwrap();
            }

            // Write short
            let (_, path_str) = path();
            {
                ctx.set_context_transaction(Some(short_request));

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
}
