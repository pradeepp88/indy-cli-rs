/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{CommandGroup, CommandGroupMetadata};

pub mod connect;
pub mod constants;
pub mod create;
pub mod delete;
pub mod disconnect;
pub mod list;
pub mod refresh;
pub mod set_protocol_version;
pub mod show_taa;

pub use self::{
    connect::*, create::*, delete::*, disconnect::*, list::*, refresh::*, set_protocol_version::*,
    show_taa::*,
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "pool",
        "Pool management commands"
    ));
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        command_executor::{CommandContext, CommandParams},
        tools::pool::Pool,
    };

    pub const POOL: &'static str = "pool";

    pub fn create_pool(ctx: &CommandContext) {
        let cmd = create_command::new();
        let mut params = CommandParams::new();
        params.insert("name", POOL.to_string());
        params.insert(
            "gen_txn_file",
            "docker_pool_transactions_genesis".to_string(),
        );
        cmd.execute(&ctx, &params).unwrap();
    }

    pub fn create_and_connect_pool(ctx: &CommandContext) {
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

        {
            let cmd = connect_command::new();
            let mut params = CommandParams::new();
            params.insert("name", POOL.to_string());
            cmd.execute(&ctx, &params).unwrap();
        }
    }

    pub fn delete_pool(ctx: &CommandContext) {
        let cmd = delete_command::new();
        let mut params = CommandParams::new();
        params.insert("name", POOL.to_string());
        cmd.execute(&ctx, &params).unwrap();
    }

    pub fn disconnect_and_delete_pool(ctx: &CommandContext) {
        {
            let cmd = disconnect_command::new();
            let params = CommandParams::new();
            cmd.execute(&ctx, &params).unwrap();
        }

        {
            let cmd = delete_command::new();
            let mut params = CommandParams::new();
            params.insert("name", POOL.to_string());
            cmd.execute(&ctx, &params).unwrap();
        }
    }

    pub fn get_pools() -> Vec<serde_json::Value> {
        let pools = Pool::list().unwrap();
        serde_json::from_str(&pools).unwrap()
    }
}
