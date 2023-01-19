/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{Command, CommandContext, CommandMetadata, CommandParams};

pub mod refresh_command {
    use super::*;

    command!(CommandMetadata::build(
        "refresh",
        "Refresh a local copy of a pool ledger and updates pool nodes connections."
    )
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ctx.ensure_connected_pool()?;

        pool.refresh()
            .map_err(|err| println_err!("Unable to refresh pool. Reason: {}", err.message(None)))?;

        println_succ!("Pool \"{}\"  has been refreshed", pool.name);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod refresh {
        use super::*;
        use crate::pool::tests::{
            create_and_connect_pool, create_pool, delete_pool, disconnect_and_delete_pool,
        };

        #[test]
        pub fn refresh_works() {
            let ctx = setup();
            create_and_connect_pool(&ctx);
            {
                let cmd = refresh_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn refresh_works_for_not_opened() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = refresh_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }
    }
}
