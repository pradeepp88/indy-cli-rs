/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams, CommandResult},
    params_parser::ParamParser,
};

pub mod prompt_command {
    use super::*;

    command!(CommandMetadata::build("prompt", "Change command prompt")
        .add_main_param("prompt", "New prompt string")
        .add_example("prompt new-prompt")
        .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> ctx: {:?}, params: {:?}", ctx, params);

        let prompt = ParamParser::get_str_param("prompt", params)?;

        ctx.set_main_prompt(prompt.to_owned());
        println_succ!("Command prompt has been set to \"{}\"", prompt);
        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}
