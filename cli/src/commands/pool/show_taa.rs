/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{Command, CommandContext, CommandMetadata, CommandParams};

pub mod show_taa_command {
    use super::*;
    use crate::pool::set_transaction_author_agreement;

    command!(CommandMetadata::build(
        "show-taa",
        "Show transaction author agreement set on Ledger."
    )
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ctx.ensure_connected_pool()?;

        match set_transaction_author_agreement(ctx, &pool, false) {
            Err(_) => (),
            Ok(Some(_)) => (),
            Ok(None) => {
                println!("There is no transaction agreement set on the Pool.");
            }
        };

        trace!("execute <<");
        Ok(())
    }
}
