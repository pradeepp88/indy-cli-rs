/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{
        wait_for_user_reply, Command, CommandContext, CommandMetadata, CommandParams,
        DynamicCompletionType,
    },
    ledger::get_active_transaction_author_agreement,
    params_parser::ParamParser,
    tools::pool::Pool,
};

use chrono::prelude::*;
use indy_vdr::{config::PoolConfig, pool::ProtocolVersion};

pub mod connect_command {
    use super::*;
    use crate::pool::close_pool;

    command_with_cleanup!(CommandMetadata::build(
        "connect",
        "Connect to pool with specified name. Also disconnect from previously connected."
    )
    .add_main_param_with_dynamic_completion("name", "The name of pool", DynamicCompletionType::Pool)
    .add_optional_param(
        "protocol-version",
        "Pool protocol version will be used for requests. One of: 1, 2. (2 by default)"
    )
    .add_optional_param("timeout", "Timeout for network request (in sec)")
    .add_optional_param(
        "extended-timeout",
        "Extended timeout for network request (in sec)"
    )
    .add_optional_param(
        "pre-ordered-nodes",
        "Names of nodes which will have a priority during request sending"
    )
    .add_optional_param(
        "number-read-nodes",
        "The number of nodes to send read requests (2 by default)"
    )
    .add_example("pool connect pool1")
    .add_example("pool connect pool1 protocol-version=2")
    .add_example("pool connect pool1 protocol-version=2 timeout=100")
    .add_example("pool connect pool1 protocol-version=2 extended-timeout=100")
    .add_example("pool connect pool1 protocol-version=2 pre-ordered-nodes=Node2,Node1")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let name = ParamParser::get_str_param("name", params)?;
        let protocol_version =
            ParamParser::get_opt_number_param::<usize>("protocol-version", params)?
                .unwrap_or(ctx.get_pool_protocol_version());
        let timeout = ParamParser::get_opt_number_param::<i64>("timeout", params)?;
        let extended_timeout =
            ParamParser::get_opt_number_param::<i64>("extended-timeout", params)?;
        let pre_ordered_nodes = ParamParser::get_opt_str_array_param("pre-ordered-nodes", params)?;
        let number_read_nodes =
            ParamParser::get_opt_number_param::<usize>("number-read-nodes", params)?;
        let protocol_version = ProtocolVersion::from_id(protocol_version as i64).map_err(|_| {
            println_err!("Unexpected Pool protocol version \"{}\".", protocol_version)
        })?;

        let config = PoolConfig {
            protocol_version,
            ack_timeout: timeout.unwrap_or(PoolConfig::default_ack_timeout()),
            reply_timeout: extended_timeout.unwrap_or(PoolConfig::default_reply_timeout()),
            request_read_nodes: number_read_nodes
                .unwrap_or(PoolConfig::default_request_read_nodes()),
            ..PoolConfig::default()
        };

        if let Some(pool) = ctx.get_connected_pool() {
            close_pool(ctx, &pool)?;
        }

        let pool = Pool::open(name, config, pre_ordered_nodes)
            .map_err(|err| println_err!("{}", err.message(Some(&name))))?;

        ctx.set_connected_pool(pool);
        println_succ!("Pool \"{}\" has been connected", name);

        let pool = ctx.ensure_connected_pool()?;
        set_transaction_author_agreement(ctx, &pool, true)?;

        trace!("execute <<");
        Ok(())
    }

    pub fn cleanup(ctx: &CommandContext) {
        trace!("cleanup >> ctx {:?}", ctx);

        if let Some(pool) = ctx.get_connected_pool() {
            close_pool(ctx, &pool).ok();
        }

        trace!("cleanup <<");
    }
}

pub fn accept_transaction_author_agreement(ctx: &CommandContext, text: &str, version: &str) {
    println!("Would you like to accept it? (y/n)");

    let accept_agreement = wait_for_user_reply(ctx);

    if !accept_agreement {
        println_warn!("The Transaction Author Agreement has NOT been Accepted.");
        println!("Use `pool show-taa` command to accept the Agreement.");
        println!();
        return;
    }

    println_succ!("Transaction Author Agreement has been accepted.");

    let time_of_acceptance = Utc::now().timestamp() as u64;

    ctx.set_transaction_author_info(Some((
        text.to_string(),
        version.to_string(),
        time_of_acceptance,
    )));
}

pub fn set_transaction_author_agreement(
    ctx: &CommandContext,
    pool: &Pool,
    ask_for_showing: bool,
) -> Result<Option<()>, ()> {
    if let Some((text, version, digest)) = get_active_transaction_author_agreement(pool)? {
        if ask_for_showing {
            println!();
            println!("There is a Transaction Author Agreement set on the connected Pool.");
            println!("You should read and accept it to be able to send transactions to the Pool.");
            println!("You can postpone accepting the Agreement. Accept it later by calling `pool show-taa` command");
            println!("Would you like to read it? (y/n)");

            let read_agreement = wait_for_user_reply(ctx);

            if !read_agreement {
                println_warn!("The Transaction Author Agreement has NOT been Accepted.");
                println!("Use `pool show-taa` command to accept the Agreement.");
                println!();
                return Ok(Some(()));
            }
        }

        println!("Transaction Author Agreement");
        println!("Version: {:?}", version);
        if let Some(digest_) = digest {
            println!("Digest: {:?}", digest_);
        }
        println!("Content: \n {:?}", text);

        accept_transaction_author_agreement(ctx, &text, &version);

        Ok(Some(()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod connect {
        use super::*;
        use crate::pool::tests::{
            create_and_connect_pool, create_pool, delete_pool, disconnect_and_delete_pool, POOL,
        };

        #[test]
        pub fn connect_works() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ctx.ensure_connected_pool().unwrap();
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_twice() {
            let ctx = setup();
            create_and_connect_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_not_created() {
            let ctx = setup();
            let cmd = connect_command::new();
            let mut params = CommandParams::new();
            params.insert("name", POOL.to_string());
            cmd.execute(&ctx, &params).unwrap_err();
            tear_down();
        }

        #[test]
        pub fn connect_works_for_invalid_protocol_version() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert("protocol-version", "0".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_incompatible_protocol_version() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert("protocol-version", "1".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_timeout() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert("timeout", "100".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ctx.ensure_connected_pool().unwrap();
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_extended_timeout() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert("extended-timeout", "100".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ctx.ensure_connected_pool().unwrap();
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn connect_works_for_pre_orded_nodes() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = connect_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                params.insert("pre-ordered-nodes", "Node2,Node1".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ctx.ensure_connected_pool().unwrap();
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }
    }
}
