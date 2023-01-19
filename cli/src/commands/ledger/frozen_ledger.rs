/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response},
    utils::table::print_list_table,
};

use serde_json::Value as JsonValue;

use super::common::handle_transaction_response;

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
        let ledgers_ids = ParamParser::get_number_tuple_array_param("ledgers_ids", params);
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let wallet = ctx.ensure_opened_wallet()?;

        let mut request =
            Ledger::build_ledgers_freeze_request(pool.as_deref(), &submitter_did, ledgers_ids?)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) =
            send_write_request!(&ctx, params, &mut request, &wallet, &submitter_did);

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

        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let request = Ledger::build_get_frozen_ledgers_request(pool.as_deref(), &submitter_did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request);
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

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
}
