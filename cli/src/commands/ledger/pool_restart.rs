/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response},
    utils::table::print_table,
};

use serde_json::Value as JsonValue;
use std::collections::HashMap;

use super::common::{handle_transaction_response, sign_and_submit_action};

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

        let pool = ctx.ensure_connected_pool()?;
        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;

        let action = ParamParser::get_str_param("action", params)?;
        let datetime = ParamParser::get_opt_str_param("datetime", params)?;
        let nodes = ParamParser::get_opt_str_array_param("nodes", params)?;
        let timeout = ParamParser::get_opt_number_param::<i64>("timeout", params)?;

        let mut request =
            Ledger::indy_build_pool_restart_request(Some(&pool), &submitter_did, action, datetime)
                .map_err(|err| println_err!("{}", err.message(Some(&pool.name))))?;

        let response = if nodes.is_some() || timeout.is_some() {
            sign_and_submit_action(&wallet, &pool, &submitter_did, &mut request, nodes, timeout)
                .map_err(|err| println_err!("{}", err.message(None)))?
        } else {
            Ledger::sign_and_submit_request(&pool, &wallet, &submitter_did, &mut request)
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::tests::use_trustee,
    };

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
}
