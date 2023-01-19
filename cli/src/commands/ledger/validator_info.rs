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
use std::collections::BTreeMap;

use super::common::{handle_transaction_response, sign_and_submit_action};

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

        let pool = ctx.ensure_connected_pool()?;
        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;

        let nodes = ParamParser::get_opt_str_array_param("nodes", params)?;
        let timeout = ParamParser::get_opt_number_param::<i64>("timeout", params)?;

        let mut request = Ledger::build_get_validator_info_request(Some(&pool), &submitter_did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let response = if nodes.is_some() || timeout.is_some() {
            sign_and_submit_action(&wallet, &pool, &submitter_did, &mut request, nodes, timeout)
                .map_err(|err| println_err!("{}", err.message(None)))?
        } else {
            Ledger::sign_and_submit_request(&pool, &wallet, &submitter_did, &mut request)
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::tests::use_trustee,
    };

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
}
