/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams, CommandResult},
    params_parser::ParamParser,
    utils::logger,
};

pub mod init_logger_command {
    use super::*;

    command!(CommandMetadata::build("init-logger", "Init logger according to a config file. \n\tIndy Cli uses `log4rs` logging framework: https://crates.io/crates/log4rs")
                            .add_main_param("file", "The path to the logger config file")
                            .add_example("init-logger /home/logger.yml")
                            .finalize());

    fn execute(_ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> params: {:?}", params);

        let file = ParamParser::get_str_param("file", params)?;

        match logger::IndyCliLogger::init(&file) {
            Ok(()) => println_succ!(
                "Logger has been initialized according to the config file: \"{}\"",
                file
            ),
            Err(err) => println_err!("{}", err),
        };

        trace!("execute << ");

        Ok(())
    }
}
