/*
    Copyright © 2023 Province of British Columbia
    https://digital.gov.bc.ca/digital-trust
*/
use crate::command_executor::{
    Command, CommandContext, CommandMetadata, CommandParams, CommandResult,
};

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
