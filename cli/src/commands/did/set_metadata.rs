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

pub mod set_metadata_command {
    use super::*;

    command!(
        CommandMetadata::build("set-metadata", "Updated metadata for a DID in the wallet.")
            .add_required_param_with_dynamic_completion(
                "did",
                "Did stored in wallet",
                DynamicCompletionType::Did
            )
            .add_required_param("metadata", "Metadata to set.")
            .add_example(r#"did set-metadata did=VsKV7grR1BUE29mG2Fm2kX metadata={"label":"Main"}"#)
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?}, params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let did = ParamParser::get_did_param("did", params)?;
        let metadata = ParamParser::get_str_param("metadata", params)?;

        Did::set_metadata(&wallet, &did, metadata)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        println_succ!("DID Metadata updated");

        trace!("execute <<");
        Ok(())
    }
}
