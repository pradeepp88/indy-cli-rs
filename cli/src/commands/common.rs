use crate::command_executor::{
    Command, CommandContext, CommandMetadata, CommandParams, CommandResult,
};
use crate::commands::get_str_param;

use crate::utils::file::read_file;
use crate::utils::logger;

pub mod about_command {
    use super::*;

    command!(CommandMetadata::build("about", "Show about information").finalize());

    fn execute(_ctx: &CommandContext, _params: &CommandParams) -> CommandResult {
        trace!("execute >> _ctx: params: {:?}", _params);

        println_succ!("Hyperledger Aries Indy CLI (https://github.com/hyperledger/indy-cli-rs.git)");
        println!();
        println_succ!("This is CLI tool for Hyperledger Indy (https://www.hyperledger.org/projects),");
        println_succ!("which provides a distributed-ledger-based foundation for");
        println_succ!("self-sovereign identity (https://sovrin.org/).");
        println!();
        println_succ!("Version: {}", env!("CARGO_PKG_VERSION"));
        println_succ!("Apache License Version 2.0");
        println_succ!("Copyright 2023 Hyperledger Aries");
        println!();

        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}

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

        let file = get_str_param("file", params).map_err(error_err!())?;

        let content = read_file(file).map_err(|err| println_err!("{}", err))?;

        println!("{}", content);
        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}

pub mod prompt_command {
    use super::*;

    command!(CommandMetadata::build("prompt", "Change command prompt")
        .add_main_param("prompt", "New prompt string")
        .add_example("prompt new-prompt")
        .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> ctx: {:?}, params: {:?}", ctx, params);

        let prompt = get_str_param("prompt", params).map_err(error_err!())?;

        ctx.set_main_prompt(prompt.to_owned());
        println_succ!("Command prompt has been set to \"{}\"", prompt);
        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}

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

pub mod init_logger_command {
    use super::*;

    command!(CommandMetadata::build("init-logger", "Init logger according to a config file. \n\tIndy Cli uses `log4rs` logging framework: https://crates.io/crates/log4rs")
                            .add_main_param("file", "The path to the logger config file")
                            .add_example("init-logger /home/logger.yml")
                            .finalize());

    fn execute(_ctx: &CommandContext, params: &CommandParams) -> CommandResult {
        trace!("execute >> params: {:?}", params);

        let file = get_str_param("file", params).map_err(error_err!())?;

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

pub mod exit_command {
    use super::*;

    command!(CommandMetadata::build("exit", "Exit Indy CLI").finalize());

    fn execute(ctx: &CommandContext, _params: &CommandParams) -> CommandResult {
        trace!("execute >> ctx: {:?}, params: {:?}", ctx, _params);

        ctx.set_exit();
        let res = Ok(());

        trace!("execute << {:?}", res);
        res
    }
}
