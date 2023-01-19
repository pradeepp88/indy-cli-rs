/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    tools::pool::Pool,
};

pub mod disconnect_command {
    use super::*;

    command!(CommandMetadata::build("disconnect", "Disconnect from current pool.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ctx.ensure_connected_pool()?;

        close_pool(ctx, &pool)?;

        trace!("execute <<");
        Ok(())
    }
}

pub fn close_pool(ctx: &CommandContext, pool: &Pool) -> Result<(), ()> {
    pool.close()
        .map(|_| {
            ctx.reset_connected_pool();
            ctx.set_transaction_author_info(None);
            println_succ!("Pool \"{}\" has been disconnected", pool.name)
        })
        .map_err(|err| println_err!("{}", err.message(Some(&pool.name))))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod disconnect {
        use super::*;
        use crate::pool::tests::{create_and_connect_pool, create_pool, delete_pool};

        #[test]
        pub fn disconnect_works_for_not_opened() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = disconnect_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn disconnect_works_for_twice() {
            let ctx = setup();
            create_and_connect_pool(&ctx);
            {
                let cmd = disconnect_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = disconnect_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }
    }
}
