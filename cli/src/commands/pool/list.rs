/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    tools::pool::Pool,
    utils::table::print_list_table,
};

pub mod list_command {
    use super::*;

    command!(CommandMetadata::build("list", "List existing pool configs.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pools = Pool::list().map_err(|err| println_err!("{}", err.message(None)))?;

        let pools: Vec<serde_json::Value> = serde_json::from_str(&pools)
            .map_err(|_| println_err!("Wrong data has been received"))?;

        print_list_table(&pools, &[("pool", "Pool")], "There are no pools defined");

        if let Some(pool) = ctx.get_connected_pool() {
            println_succ!("Current pool \"{}\"", pool.name);
        }

        trace!("execute <<");
        Ok(())
    }
}

pub fn pool_list() -> Vec<String> {
    Pool::list()
        .ok()
        .and_then(|pools| serde_json::from_str::<Vec<serde_json::Value>>(&pools).ok())
        .unwrap_or(vec![])
        .into_iter()
        .map(|pool| {
            pool["pool"]
                .as_str()
                .map(String::from)
                .unwrap_or(String::new())
        })
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod list {
        use super::*;
        use crate::pool::tests::{create_pool, delete_pool};

        #[test]
        pub fn list_works() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn list_works_for_empty_list() {
            let ctx = setup();
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down();
        }
    }
}
