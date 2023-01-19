/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{
    Command, CommandContext, CommandMetadata, CommandParams, CommandResult,
};

pub mod about_command {
    use super::*;

    command!(CommandMetadata::build("about", "Show about information").finalize());

    fn execute(_ctx: &CommandContext, _params: &CommandParams) -> CommandResult {
        trace!("execute >> _ctx: params: {:?}", _params);

        println_succ!(
            "Hyperledger Aries Indy CLI (https://github.com/hyperledger/indy-cli-rs.git)"
        );
        println!();
        println_succ!(
            "This is CLI tool for Hyperledger Indy (https://www.hyperledger.org/projects),"
        );
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
