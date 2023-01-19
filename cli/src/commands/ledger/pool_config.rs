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

use super::common::{handle_transaction_response, print_transaction_response};

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

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let writes = ParamParser::get_bool_param("writes", params)?;
        let force = ParamParser::get_opt_bool_param("force", params)?.unwrap_or(false);

        let mut request =
            Ledger::indy_build_pool_config_request(pool.as_deref(), &submitter_did, writes, force)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::tests::use_trustee,
    };

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
}
