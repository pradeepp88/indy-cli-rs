use crate::{
    command_executor::{
        wait_for_user_reply, Command, CommandContext, CommandGroup, CommandGroupMetadata,
        CommandMetadata, CommandParams, DynamicCompletionType,
    },
    commands::*,
    tools::pool::Pool,
    utils::table::print_list_table,
};

use chrono::prelude::*;
use indy_vdr::{config::PoolConfig, pool::ProtocolVersion};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "pool",
        "Pool management commands"
    ));
}

pub mod create_command {
    use super::*;
    use crate::utils::pool_config::Config;

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

        let name = get_str_param("name", params).map_err(error_err!())?;
        let gen_txn_file = get_str_param("gen_txn_file", params).map_err(error_err!())?;

        trace!(
            r#"Pool::create_pool_ledger_config try: name {}, gen_txn_file {:?}"#,
            name,
            gen_txn_file
        );

        let config = Config {
            genesis_txn: gen_txn_file.to_string(),
        };

        Pool::create_config(name, &config)
            .map_err(|err| println_err!("{}", err.message(Some(&name))))?;

        println_succ!("Pool config \"{}\" has been created", name);

        trace!("execute <<");
        Ok(())
    }
}

pub mod connect_command {
    use super::*;

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

        let name = get_str_param("name", params).map_err(error_err!())?;
        let protocol_version = get_opt_number_param::<usize>("protocol-version", params)
            .map_err(error_err!())?
            .unwrap_or(get_pool_protocol_version(ctx));
        let timeout = get_opt_number_param::<i64>("timeout", params).map_err(error_err!())?;
        let extended_timeout =
            get_opt_number_param::<i64>("extended-timeout", params).map_err(error_err!())?;
        let _pre_ordered_nodes =
            get_opt_str_array_param("pre-ordered-nodes", params).map_err(error_err!())?;
        let number_read_nodes =
            get_opt_number_param::<usize>("number-read-nodes", params).map_err(error_err!())?;

        // let config = {
        //     let mut json = JSONMap::new();
        //     update_json_map_opt_key!(json, "timeout", timeout);
        //     update_json_map_opt_key!(json, "extended_timeout", extended_timeout);
        //     update_json_map_opt_key!(json, "preordered_nodes", pre_ordered_nodes);
        //     update_json_map_opt_key!(json, "number_read_nodes", number_read_nodes);
        //     JSONValue::from(json).to_string()
        // };

        let protocol_version = ProtocolVersion::from_id(protocol_version as i64).map_err(|_| {
            println_err!("Unexpected Pool protocol version \"{}\".", protocol_version)
        })?;

        // TODO: Clarify settings
        let config = PoolConfig {
            protocol_version,
            reply_timeout: timeout.unwrap_or(PoolConfig::default_reply_timeout()),
            ack_timeout: extended_timeout.unwrap_or(PoolConfig::default_ack_timeout()),
            request_read_nodes: number_read_nodes
                .unwrap_or(PoolConfig::default_request_read_nodes()),
            ..PoolConfig::default()
        };

        if let Some((pool, name)) = get_connected_pool_with_name(ctx) {
            close_pool(ctx, &pool, &name)?;
        }

        let pool =
            Pool::open(name, config).map_err(|err| println_err!("{}", err.message(Some(&name))))?;

        set_connected_pool(ctx, Some((pool, name.to_owned())));
        println_succ!("Pool \"{}\" has been connected", name);

        let pool = ensure_connected_pool(ctx)?;
        set_transaction_author_agreement(ctx, &pool, true)?;

        trace!("execute <<");
        Ok(())
    }

    pub fn cleanup(ctx: &CommandContext) {
        trace!("cleanup >> ctx {:?}", ctx);

        if let Some((pool, name)) = get_connected_pool_with_name(ctx) {
            close_pool(ctx, &pool, &name).ok();
        }

        trace!("cleanup <<");
    }
}

pub mod list_command {
    use super::*;

    command!(CommandMetadata::build("list", "List existing pool configs.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pools = Pool::list().map_err(|err| println_err!("{}", err.message(None)))?;

        let pools: Vec<serde_json::Value> = serde_json::from_str(&pools)
            .map_err(|_| println_err!("Wrong data has been received"))?;

        print_list_table(&pools, &[("pool", "Pool")], "There are no pools defined");

        if let Some((_, cur_pool)) = get_connected_pool_with_name(ctx) {
            println_succ!("Current pool \"{}\"", cur_pool);
        }

        trace!("execute <<");
        Ok(())
    }
}

pub mod show_taa_command {
    use super::*;

    command!(CommandMetadata::build(
        "show-taa",
        "Show transaction author agreement set on Ledger."
    )
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ensure_connected_pool_handle(&ctx)?;

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

pub mod refresh_command {
    use super::*;

    command!(CommandMetadata::build(
        "refresh",
        "Refresh a local copy of a pool ledger and updates pool nodes connections."
    )
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ensure_connected_pool(&ctx)?;
        let pool_name = ensure_connected_pool_name(&ctx)?;

        Pool::refresh(&pool_name, &pool)
            .map_err(|err| println_err!("Unable to refresh pool. Reason: {}", err.message(None)))?;

        println_succ!("Pool \"{}\"  has been refreshed", pool_name);

        trace!("execute <<");
        Ok(())
    }
}

pub mod set_protocol_version_command {
    use super::*;

    command!(CommandMetadata::build(
        "set-protocol-version",
        "Set protocol version that will be used for ledger requests. One of: 1, 2. \
                 Unless command is called the default protocol version 2 is used."
    )
    .add_main_param("protocol-version", "Protocol version to use")
    .add_example("pool set-protocol-version 2")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let protocol_version =
            get_number_param::<usize>("protocol-version", params).map_err(error_err!())?;

        set_pool_protocol_version(ctx, protocol_version);
        println_succ!("Protocol Version has been set: \"{}\".", protocol_version);

        trace!("execute <<");
        Ok(())
    }
}

pub mod disconnect_command {
    use super::*;

    command!(CommandMetadata::build("disconnect", "Disconnect from current pool.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let pool = ensure_connected_pool(&ctx)?;
        let name = ensure_connected_pool_name(&ctx)?;

        close_pool(ctx, &pool, &name)?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod delete_command {
    use super::*;

    command!(
        CommandMetadata::build("delete", "Delete pool config with specified name")
            .add_main_param_with_dynamic_completion(
                "name",
                "The name of deleted pool config",
                DynamicCompletionType::Pool
            )
            .add_example("pool delete pool1")
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let name = get_str_param("name", params).map_err(error_err!())?;

        trace!(r#"Pool::delete try: name {}"#, name);

        if let Some((pool, name)) = get_connected_pool_with_name(ctx) {
            close_pool(ctx, &pool, &name)?;
        }

        Pool::delete(name).map_err(|err| println_err!("{}", err.message(Some(&name))))?;

        println_succ!("Pool \"{}\" has been deleted.", name);

        trace!("execute <<");
        Ok(())
    }
}

fn close_pool(ctx: &CommandContext, pool: &LocalPool, name: &str) -> Result<(), ()> {
    Pool::close(pool)
        .map(|_| {
            set_connected_pool(ctx, None);
            set_transaction_author_info(ctx, None);
            println_succ!("Pool \"{}\" has been disconnected", name)
        })
        .map_err(|err| println_err!("{}", err.message(Some(&name))))
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

    set_transaction_author_info(
        ctx,
        Some((text.to_string(), version.to_string(), time_of_acceptance)),
    );
}

pub fn set_transaction_author_agreement(
    ctx: &CommandContext,
    pool: &LocalPool,
    ask_for_showing: bool,
) -> Result<Option<()>, ()> {
    if let Some((text, version, digest)) = ledger::get_active_transaction_author_agreement(pool)? {
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
    use crate::tools::pool::Pool;

    const POOL: &'static str = "pool";

    mod create {
        use super::*;

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

    mod connect {
        use super::*;

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
            ensure_connected_pool_handle(&ctx).unwrap();
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
            ensure_connected_pool_handle(&ctx).unwrap();
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
            ensure_connected_pool_handle(&ctx).unwrap();
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
            ensure_connected_pool_handle(&ctx).unwrap();
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }
    }

    mod list {
        use super::*;

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

    mod refresh {
        use super::*;

        #[ignore]
        // FIXME: For some reason refresh does not work with with VON network but works with Staging and Prod
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

    mod disconnect {
        use super::*;

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

    mod delete {
        use super::*;

        #[test]
        pub fn delete_works() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let pools = get_pools();
            assert_eq!(0, pools.len());

            tear_down();
        }

        #[test]
        pub fn delete_works_for_not_created() {
            let ctx = setup();

            let cmd = delete_command::new();
            let mut params = CommandParams::new();
            params.insert("name", POOL.to_string());
            cmd.execute(&ctx, &params).unwrap_err();

            tear_down();
        }

        #[test]
        pub fn delete_works_for_connected() {
            let ctx = setup();
            create_and_connect_pool(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", POOL.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down();
        }
    }

    mod set_protocol_version {
        use super::*;

        #[test]
        pub fn set_protocol_version_works() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "protocol-version",
                    DEFAULT_POOL_PROTOCOL_VERSION.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert("protocol-version", "invalid".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }
    }

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

    fn get_pools() -> Vec<serde_json::Value> {
        let pools = Pool::list().unwrap();
        serde_json::from_str(&pools).unwrap()
    }
}
