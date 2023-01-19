/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::pool::Pool,
};

pub mod create_command {
    use super::*;
    use crate::tools::pool::directory::PoolConfig;

    command!(CommandMetadata::build(
        "create",
        "Create new pool ledger config with specified name"
    )
    .add_main_param("name", "The name of new pool ledger config")
    .add_required_param("gen_txn_file", "Path to file with genesis transactions")
    .add_example("pool create pool1 gen_txn_file=/home/pool_genesis_transactions")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let name = ParamParser::get_str_param("name", params)?;
        let gen_txn_file = ParamParser::get_str_param("gen_txn_file", params)?;

        trace!(
            r#"Pool::create_pool_ledger_config try: name {}, gen_txn_file {:?}"#,
            name,
            gen_txn_file
        );

        let config = PoolConfig {
            genesis_txn: gen_txn_file.to_string(),
        };

        Pool::create(name, &config).map_err(|err| println_err!("{}", err.message(Some(&name))))?;

        println_succ!("Pool config \"{}\" has been created", name);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod create {
        use super::*;
        use crate::pool::tests::{create_pool, delete_pool, get_pools, POOL};

        #[test]
        pub fn create_works() {
            let ctx = setup();

            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert(
                    "gen_txn_file",
                    "docker_pool_transactions_genesis".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }

            let pools = get_pools();
            assert_eq!(1, pools.len());
            assert_eq!(pools[0]["pool"].as_str().unwrap(), POOL);

            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn create_works_for_twice() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert(
                    "gen_txn_file",
                    "docker_pool_transactions_genesis".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn create_works_for_missed_gen_txn_file() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test]
        pub fn create_works_for_unknown_txn_file() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert(
                    "gen_txn_file",
                    "unknown_pool_transactions_genesis".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }
    }
}
