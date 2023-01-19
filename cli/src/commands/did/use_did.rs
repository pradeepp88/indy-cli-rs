/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{
        Command, CommandContext, CommandMetadata, CommandParams, DynamicCompletionType,
    },
    params_parser::ParamParser,
    tools::did::Did,
};

pub mod use_command {
    use super::*;

    command!(CommandMetadata::build("use", "Use DID")
        .add_main_param_with_dynamic_completion(
            "did",
            "Did stored in wallet",
            DynamicCompletionType::Did
        )
        .add_example("did use VsKV7grR1BUE29mG2Fm2kX")
        .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?}, params {:?}", ctx, params);

        let did = ParamParser::get_did_param("did", params)?;

        let store = ctx.ensure_opened_wallet()?;

        Did::get(&store, &did).map_err(|err| println_err!("{}", err.message(None)))?;

        println_succ!("Did \"{}\" has been set as active", did);
        ctx.set_active_did(did);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    mod did_use {
        use super::*;
        use crate::{
            commands::{setup_with_wallet, tear_down, tear_down_with_wallet},
            did::{
                new_command,
                tests::{new_did, DID_TRUSTEE, SEED_TRUSTEE},
            },
            wallet::tests::close_and_delete_wallet,
        };

        #[test]
        pub fn use_works() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            {
                let cmd = use_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert_eq!(ctx.ensure_active_did().unwrap().to_string(), DID_TRUSTEE);
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn use_works_for_unknown_did() {
            let ctx = setup_with_wallet();
            {
                let cmd = use_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn use_works_for_closed_wallet() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            close_and_delete_wallet(&ctx);
            {
                let cmd = new_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }
    }
}
