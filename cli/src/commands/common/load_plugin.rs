/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{
    Command, CommandContext, CommandMetadata, CommandParams, CommandResult,
};

pub mod load_plugin_command {
    use super::*;

    command!(
        CommandMetadata::build("load-plugin", "Load plugin in Libindy")
            .add_required_param(
                "library",
                "Name of plugin (can be absolute or relative path)"
            )
            .add_required_param("initializer", "Name of plugin init function")
            .add_example("load-plugin library=libpostgre initializer=libpostgre_init")
            .finalize()
    );

    fn execute(_ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> params: {:?}", params);
        println_warn!("Command DEPRECATED!");
        trace!("execute << ");
        Ok(())
    }
}
