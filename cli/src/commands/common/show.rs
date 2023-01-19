/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams, CommandResult},
    params_parser::ParamParser,
    utils::file::read_file,
};

pub mod show_command {
    use super::*;

    command!(
        CommandMetadata::build("show", "Print the content of text file")
            .add_main_param("file", "The path to file to show")
            .add_example("show /home/file.txt")
            .finalize()
    );

    fn execute(_ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> params: {:?}", params);

        let file = ParamParser::get_str_param("file", params)?;

        let content = read_file(file).map_err(|err| println_err!("{}", err))?;

        println!("{}", content);
        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}
