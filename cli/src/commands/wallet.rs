use crate::command_executor::{
    Command, CommandContext, CommandGroup, CommandGroupMetadata, CommandMetadata, CommandParams,
    DynamicCompletionType,
};
use crate::commands::*;
use crate::tools::wallet::{Credentials, Wallet};
use crate::utils::{
    table::print_list_table,
    wallet_config::{Config, WalletConfig},
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "wallet",
        "Wallet management commands"
    ));
}

pub mod create_command {
    use super::*;

    command!(CommandMetadata::build("create", "Create new wallet and attach to Indy CLI")
                .add_main_param("name", "Identifier of the wallet")
                .add_required_deferred_param("key", "Key or passphrase used for wallet key derivation.
                                               Look to key_derivation_method param for information about supported key derivation methods.")
                .add_optional_param("key_derivation_method", "Algorithm to use for wallet key derivation. One of:
                                    argon2m - derive secured wallet key (used by default)
                                    argon2i - derive secured wallet key (less secured but faster)
                                    raw - raw wallet key provided (skip derivation)")
                .add_optional_param("storage_type", "Type of the wallet storage.")
                .add_optional_param("storage_config", "The list of key:value pairs defined by storage type.")
                .add_optional_param("storage_credentials", "The list of key:value pairs defined by storage type.")
                .add_example("wallet create wallet1 key")
                .add_example("wallet create wallet1 key storage_type=default")
                .add_example(r#"wallet create wallet1 key storage_type=default storage_config={"key1":"value1","key2":"value2"}"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;
        let key = get_str_param("key", params).map_err(error_err!())?;
        let key_derivation_method = get_opt_str_param("key_derivation_method", params)?;
        let storage_type = get_opt_str_param("storage_type", params)
            .map_err(error_err!())?
            .unwrap_or("default");
        let storage_config =
            get_opt_object_param("storage_config", params).map_err(error_err!())?;
        let storage_credentials =
            get_opt_object_param("storage_credentials", params).map_err(error_err!())?;

        let config = Config {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };
        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: None,
            rekey_derivation_method: None,
            storage_credentials,
        };

        trace!("Wallet::create_wallet try: config {:?}", config);

        Wallet::create(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(&id))))?;

        WalletConfig::store(id, &config)
            .map_err(|err| println_err!("Cannot store wallet \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been created", id);

        trace!("execute << {:?}", ());
        Ok(())
    }
}

pub mod attach_command {
    use super::*;

    command!(CommandMetadata::build("attach", "Attach existing wallet to Indy CLI")
                .add_main_param_with_dynamic_completion("name", "Identifier of the wallet", DynamicCompletionType::Wallet)
                .add_optional_param("storage_type", "Type of the wallet storage.")
                .add_optional_param("storage_config", "The list of key:value pairs defined by storage type.")
                .add_example("wallet attach wallet1")
                .add_example("wallet attach wallet1 storage_type=default")
                .add_example(r#"wallet attach wallet1 storage_type=default storage_config={"key1":"value1","key2":"value2"}"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;
        let storage_type = get_opt_str_param("storage_type", params)
            .map_err(error_err!())?
            .unwrap_or("default");
        let storage_config =
            get_opt_object_param("storage_config", params).map_err(error_err!())?;

        if WalletConfig::exists(id) {
            println_err!("Wallet \"{}\" is already attached to CLI", id);
            return Err(());
        }

        let config = Config {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };

        WalletConfig::store(id, &config)
            .map_err(|err| println_err!("Cannot store wallet \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been attached", id);

        trace!("execute << ");
        Ok(())
    }
}

pub mod open_command {
    use super::*;

    command_with_cleanup!(CommandMetadata::build("open", "Open wallet. Also close previously opened.")
                            .add_main_param_with_dynamic_completion("name", "Identifier of the wallet", DynamicCompletionType::Wallet)
                            .add_required_deferred_param("key", "Key or passphrase used for wallet key derivation.
                                               Look to key_derivation_method param for information about supported key derivation methods.")
                            .add_optional_param("key_derivation_method", "Algorithm to use for wallet key derivation. One of:
                                                argon2m - derive secured wallet key (used by default)
                                                argon2i - derive secured wallet key (less secured but faster)
                                                raw - raw key provided (skip derivation)")
                            .add_optional_deferred_param("rekey", "New key or passphrase used for wallet key derivation (will replace previous one).")
                            .add_optional_param("rekey_derivation_method", "Algorithm to use for wallet rekey derivation. One of:
                                                argon2m - derive secured wallet key (used by default)
                                                argon2i - derive secured wallet key (less secured but faster)
                                                raw - raw key provided (skip derivation)")
                            .add_optional_param("storage_credentials", "The list of key:value pairs defined by storage type.")
                            .add_example("wallet open wallet1 key")
                            .add_example("wallet open wallet1 key rekey")
                            .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;
        let key = get_str_param("key", params).map_err(error_err!())?;
        let rekey = get_opt_str_param("rekey", params).map_err(error_err!())?;
        let key_derivation_method =
            get_opt_str_param("key_derivation_method", params).map_err(error_err!())?;
        let rekey_derivation_method =
            get_opt_str_param("rekey_derivation_method", params).map_err(error_err!())?;
        let storage_credentials =
            get_opt_object_param("storage_credentials", params).map_err(error_err!())?;

        let config = WalletConfig::read(id)
            .map_err(|_| println_err!("Wallet \"{}\" isn't attached to CLI", id))?;

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: rekey.map(String::from),
            rekey_derivation_method: rekey_derivation_method.map(String::from),
            storage_credentials,
        };

        reset_active_did(ctx);

        if let Some((store, id)) = get_opened_wallet(ctx) {
            close_wallet(ctx, &store, &id)?;
        }

        let store = Wallet::open(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(&id))))?;

        set_opened_wallet(ctx, Some((store, id.to_owned())));
        println_succ!("Wallet \"{}\" has been opened", id);

        trace!("execute << {:?}", ());
        Ok(())
    }

    pub fn cleanup(ctx: &CommandContext) {
        trace!("cleanup >> ctx {:?}", ctx);

        if let Some((store, id)) = get_opened_wallet(ctx) {
            close_wallet(ctx, &store, &id).ok();
        }

        trace!("cleanup <<");
    }
}

pub mod list_command {
    use super::*;

    command!(CommandMetadata::build("list", "List attached wallets.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallets = Wallet::list();

        print_list_table(
            &wallets,
            &[("id", "Name"), ("storage_type", "Type")],
            "There are no wallets",
        );

        if let Some((_, cur_wallet)) = get_opened_wallet(ctx) {
            println_succ!("Current wallet \"{}\"", cur_wallet);
        }

        trace!("execute << ");
        Ok(())
    }
}

pub mod close_command {
    use super::*;

    command!(CommandMetadata::build("close", "Close opened wallet.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        if let Some((store, id)) = get_opened_wallet(ctx) {
            close_wallet(ctx, &store, &id)?;
        } else {
            println_err!("There is no opened wallet now");
            return Err(())
        }

        trace!("CloseCommand::execute <<");
        Ok(())
    }
}

pub mod delete_command {
    use super::*;

    command!(CommandMetadata::build("delete", "Delete wallet.")
                .add_main_param_with_dynamic_completion("name", "Identifier of the wallet", DynamicCompletionType::Wallet)
                .add_required_deferred_param("key", "Key or passphrase used for wallet key derivation.
                                               Look to key_derivation_method param for information about supported key derivation methods.")
                .add_optional_param("key_derivation_method", "Algorithm to use for wallet key derivation. One of:
                                    argon2m - derive secured wallet key (used by default)
                                    argon2i - derive secured wallet key (less secured but faster)
                                    raw - raw key provided (skip derivation)")
                .add_optional_param("storage_credentials", "The list of key:value pairs defined by storage type.")
                .add_example("wallet delete wallet1 key")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx: {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;
        let key = get_str_param("key", params).map_err(error_err!())?;
        let key_derivation_method =
            get_opt_str_param("key_derivation_method", params).map_err(error_err!())?;
        let storage_credentials =
            get_opt_object_param("storage_credentials", params).map_err(error_err!())?;

        let config = WalletConfig::read(id)
            .map_err(|_| println_err!("Wallet \"{}\" isn't attached to CLI", id))?;

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: None,
            rekey_derivation_method: None,
            storage_credentials,
        };

        if let Some((store, id)) = get_opened_wallet(ctx) {
            close_wallet(ctx, &store, &id)?;
        }

        Wallet::delete(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(id))))?;

        WalletConfig::delete(id)
            .map_err(|err| println_err!("Cannot delete \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been deleted", id);

        trace!("execute <<");
        Ok(())
    }
}

pub mod detach_command {
    use super::*;

    command!(
        CommandMetadata::build("detach", "Detach wallet from Indy CLI")
            .add_main_param_with_dynamic_completion(
                "name",
                "Identifier of the wallet",
                DynamicCompletionType::Wallet
            )
            .add_example("wallet detach wallet1")
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx: {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;

        if !WalletConfig::exists(id) {
            println_err!("Wallet \"{}\" isn't attached to CLI", id);
            return Err(());
        }

        if let Some((_, name)) = get_opened_wallet(ctx) {
            if id == name {
                println_err!("Wallet \"{}\" is opened", id);
                return Err(());
            }
        }

        WalletConfig::delete(id)
            .map_err(|err| println_err!("Cannot delete \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been deleted", id);

        trace!("execute << ");
        Ok(())
    }
}

pub mod export_command {
    use super::*;
    use crate::tools::wallet::ExportConfig;

    command!(CommandMetadata::build("export", "Export opened wallet to the file")
                .add_required_param("export_path", "Path to the export file")
                .add_required_deferred_param("export_key", "Key or passphrase used for export wallet key derivation.
                                               Look to key_derivation_method param for information about supported key derivation methods.")
                .add_optional_param("export_key_derivation_method", "Algorithm to use for export key derivation. One of:
                                    argon2m - derive secured export key (used by default)
                                    argon2i - derive secured export key (less secured but faster)
                                    raw - raw export key provided (skip derivation)")
                .add_example("wallet export export_path=/home/indy/export_wallet export_key")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let (store, wallet_name) = ensure_opened_wallet(&ctx)?;

        let export_path = get_str_param("export_path", params).map_err(error_err!())?;
        let export_key = get_str_param("export_key", params).map_err(error_err!())?;
        let export_key_derivation_method =
            get_opt_str_param("export_key_derivation_method", params).map_err(error_err!())?;

        let export_config = ExportConfig {
            path: export_path.to_string(),
            key: export_key.to_string(),
            key_derivation_method: export_key_derivation_method.map(String::from),
        };

        trace!(
            "Wallet::export_wallet try: wallet_name {}, export_path {}",
            wallet_name,
            export_path
        );

        Wallet::export(&store, &export_config)
            .map_err(|err| println_err!("{}", err.message(Some(&wallet_name))))?;

        println_succ!(
            "Wallet \"{}\" has been exported to the file \"{}\"",
            wallet_name,
            export_path
        );

        trace!("execute <<");
        Ok(())
    }
}

pub mod import_command {
    use super::*;
    use crate::tools::wallet::ImportConfig;

    command!(CommandMetadata::build("import", "Create new wallet, attach to Indy CLI and then import content from the specified file")
                .add_main_param_with_dynamic_completion("name", "The name of new wallet", DynamicCompletionType::Wallet)
                .add_required_deferred_param("key", "Key or passphrase used for wallet key derivation.
                                               Look to key_derivation_method param for information about supported key derivation methods.")
                .add_optional_param("key_derivation_method", "Algorithm to use for wallet key derivation. One of:
                                    argon2m - derive secured wallet key (used by default)
                                    argon2i - derive secured wallet key (less secured but faster)
                                    raw - raw key provided (skip derivation)")
                .add_optional_param("storage_type", "Type of the wallet storage.")
                .add_optional_param("storage_config", "The list of key:value pairs defined by storage type.")
                .add_optional_param("storage_credentials", "The list of key:value pairs defined by storage type.")
                .add_required_param("export_path", "Path to the file that contains exported wallet content")
                .add_required_deferred_param("export_key", "Key used for export of the wallet")
                .add_example("wallet import wallet1 key export_path=/home/indy/export_wallet export_key")
                .add_example(r#"wallet import wallet1 key export_path=/home/indy/export_wallet export_key storage_type=default storage_config={"key1":"value1","key2":"value2"}"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let id = get_str_param("name", params).map_err(error_err!())?;
        let key = get_str_param("key", params).map_err(error_err!())?;
        let key_derivation_method =
            get_opt_str_param("key_derivation_method", params).map_err(error_err!())?;
        let export_path = get_str_param("export_path", params).map_err(error_err!())?;
        let export_key = get_str_param("export_key", params).map_err(error_err!())?;
        let storage_type = get_opt_str_param("storage_type", params)
            .map_err(error_err!())?
            .unwrap_or("default");
        let storage_config =
            get_opt_object_param("storage_config", params).map_err(error_err!())?;
        let storage_credentials =
            get_opt_object_param("storage_credentials", params).map_err(error_err!())?;

        let config = Config {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };

        let import_config = ImportConfig {
            path: export_path.to_string(),
            key: export_key.to_string(),
        };

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: None,
            rekey_derivation_method: None,
            storage_credentials,
        };

        if WalletConfig::exists(id) {
            println_err!("Wallet \"{}\" is already attached to CLI", id);
            return Err(());
        }

        trace!(
            "Wallet::import_wallet try: config {:?}, import_config {:?}",
            config,
            secret!(&import_config)
        );

        Wallet::import(&config, &credentials, &import_config)
            .map_err(|err| println_err!("{}", err.message(Some(id))))?;

        WalletConfig::store(id, &config)
            .map_err(|err| println_err!("Cannot store \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been created", id);

        trace!("execute <<");
        Ok(())
    }
}

fn close_wallet(ctx: &CommandContext, store: &AnyStore, name: &str) -> Result<(), ()> {
    Wallet::close(store)
        .map(|_| {
            set_opened_wallet(ctx, None);
            reset_active_did(ctx);
            println_succ!("Wallet \"{}\" has been closed", name);
        })
        .map_err(|err| println_err!("{}", err.message(Some(name))))
}

pub fn wallet_names() -> Vec<String> {
    Wallet::list()
        .into_iter()
        .map(|wallet| {
            wallet["id"]
                .as_str()
                .map(String::from)
                .unwrap_or(String::new())
        })
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::utils::environment::EnvironmentUtils;
    use std::path::PathBuf;

    const WALLET: &str = "wallet";
    const WALLET_KEY: &str = "wallet_key";
    const WALLET_KEY_RAW: &str = "6nxtSiXFvBd593Y2DCed2dYvRY1PGK9WMtxCBjLzKgbw";
    const EXPORT_KEY: &str = "export_key";

    mod create {
        use super::*;

        #[test]
        pub fn create_works() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);

            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn create_works_for_twice() {
            let ctx = setup_with_wallet();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn create_works_for_missed_credentials() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test]
        pub fn create_works_for_type() {
            let ctx = setup();
            let storage_type = "default";
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                params.insert("storage_type", storage_type.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);
            assert_eq!(wallets[0]["storage_type"].as_str().unwrap(), storage_type);

            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn create_works_for_config() {
            let ctx = setup();
            let config = r#"{"key":"value","key2":"value2"}"#;
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                params.insert("storage_config", config.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);
            assert_eq!(
                wallets[0]["storage_config"].as_object().unwrap(),
                serde_json::from_str::<serde_json::Value>(config)
                    .unwrap()
                    .as_object()
                    .unwrap()
            );
            tear_down();
        }

        #[ignore]
        #[test]
        pub fn create_works_for_key_derivation_method() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "argon2m".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);

            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "argon2m".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down();
        }

        #[test]
        pub fn create_works_for_wrong_key_derivation_method() {
            let ctx = setup();
            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "some_type".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }
    }

    mod attach {
        use super::*;

        #[test]
        pub fn attach_works() {
            let ctx = setup();
            {
                let cmd = attach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());
            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);

            tear_down();
        }

        #[test]
        pub fn attach_works_for_twice() {
            let ctx = setup();
            attach_wallet(&ctx);
            {
                let cmd = attach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test]
        pub fn attach_works_for_type() {
            let ctx = setup();
            let storage_type = "default";
            {
                let cmd = attach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("storage_type", storage_type.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);
            assert_eq!(wallets[0]["storage_type"].as_str().unwrap(), storage_type);

            tear_down();
        }

        #[test]
        pub fn attach_for_config() {
            let ctx = setup();
            let config = r#"{"key":"value","key2":"value2"}"#;
            {
                let cmd = attach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("storage_config", config.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);
            assert_eq!(
                wallets[0]["storage_config"].as_object().unwrap(),
                serde_json::from_str::<serde_json::Value>(config)
                    .unwrap()
                    .as_object()
                    .unwrap()
            );

            tear_down();
        }
    }

    mod open {
        use super::*;

        #[test]
        pub fn open_works() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ensure_opened_store(&ctx).unwrap();
            close_and_delete_wallet(&ctx);

            tear_down();
        }

        #[test]
        pub fn open_works_for_twice() {
            let ctx = setup_with_wallet();
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap(); //TODO: we close and open same wallet
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn open_works_for_not_created() {
            let ctx = setup();
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test]
        pub fn open_works_for_missed_key() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn open_works_for_wrong_key() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", "other_key".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }
    }

    mod list {
        use super::*;

        #[test]
        pub fn list_works() {
            let ctx = setup_with_wallet();
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
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

    mod close {
        use super::*;

        #[test]
        pub fn close_works() {
            let ctx = setup_with_wallet();
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            ensure_opened_store(&ctx).unwrap_err();
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn close_works_for_not_opened() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn close_works_for_twice() {
            let ctx = setup();
            create_and_open_wallet(&ctx);
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }
    }

    mod delete {
        use super::*;

        #[test]
        pub fn delete_works() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }
            let wallets = Wallet::list();
            assert_eq!(0, wallets.len());

            tear_down();
        }

        #[test]
        pub fn delete_works_for_not_created() {
            let ctx = setup();

            let cmd = delete_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY.to_string());
            cmd.execute(&ctx, &params).unwrap_err();

            tear_down();
        }

        #[test]
        pub fn delete_works_for_opened() {
            let ctx = setup();
            create_and_open_wallet(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down();
        }

        #[ignore]
        #[test]
        pub fn delete_works_for_wrong_key() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", "other_key".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }
    }

    mod detach {
        use super::*;

        #[test]
        pub fn detach_works() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = detach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(0, wallets.len());

            attach_wallet(&ctx);
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn detach_works_for_not_attached() {
            let ctx = setup();

            let cmd = detach_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            cmd.execute(&ctx, &params).unwrap_err();

            tear_down();
        }

        #[test]
        pub fn detach_works_for_opened() {
            let ctx = setup();

            create_and_open_wallet(&ctx);
            {
                let cmd = detach_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            close_and_delete_wallet(&ctx);
            tear_down();
        }
    }

    mod export {
        use super::*;

        #[ignore]
        #[test]
        pub fn export_works() {
            let ctx = setup_with_wallet();

            let (path, path_str) = export_wallet_path();
            {
                let cmd = export_command::new();
                let mut params = CommandParams::new();
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            assert!(path.exists());
            tear_down_with_wallet(&ctx);
        }

        #[ignore]
        #[test]
        pub fn export_works_for_file_already_exists() {
            let ctx = setup_with_wallet();

            let (_, path_str) = export_wallet_path();

            export_wallet(&ctx, &path_str);
            {
                let cmd = export_command::new();
                let mut params = CommandParams::new();
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }
    }

    mod import {
        use super::did::tests::{new_did, use_did, DID_MY1, SEED_MY1};
        use super::*;

        #[ignore]
        #[test]
        pub fn import_works() {
            let ctx = setup();

            create_and_open_wallet(&ctx);
            new_did(&ctx, SEED_MY1);
            use_did(&ctx, DID_MY1);

            let (_, path_str) = export_wallet_path();
            export_wallet(&ctx, &path_str);

            let wallet_name = "imported_wallet";
            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            // open exported wallet
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            use_did(&ctx, DID_MY1);

            close_and_delete_wallet(&ctx);

            // delete first wallet
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY.to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }

            tear_down();
        }

        #[ignore]
        #[test]
        pub fn import_works_for_not_found_file() {
            let ctx = setup();
            let (_, path_str) = export_wallet_path();
            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[ignore]
        #[test]
        pub fn import_works_for_other_key() {
            let ctx = setup();

            create_and_open_wallet(&ctx);
            new_did(&ctx, SEED_MY1);
            use_did(&ctx, DID_MY1);

            let (_, path_str) = export_wallet_path();
            export_wallet(&ctx, &path_str);

            let wallet_name = "imported_wallet";
            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", "other_key".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            close_and_delete_wallet(&ctx);
            tear_down();
        }

        #[ignore]
        #[test]
        pub fn import_works_for_duplicate_name() {
            let ctx = setup();

            create_and_open_wallet(&ctx);

            let (_, path_str) = export_wallet_path();
            export_wallet(&ctx, &path_str);

            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            close_and_delete_wallet(&ctx);
            tear_down();
        }

        #[ignore]
        #[test]
        pub fn import_works_for_config() {
            let ctx = setup();

            create_and_open_wallet(&ctx);
            new_did(&ctx, SEED_MY1);
            use_did(&ctx, DID_MY1);

            let (_, path_str) = export_wallet_path();
            export_wallet(&ctx, &path_str);
            close_and_delete_wallet(&ctx);

            let config = r#"{"key":"value"}"#;

            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert("storage_config", config.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallets = Wallet::list();
            assert_eq!(1, wallets.len());

            assert_eq!(wallets[0]["id"].as_str().unwrap(), WALLET);
            assert_eq!(
                wallets[0]["storage_config"].as_object().unwrap(),
                serde_json::from_str::<serde_json::Value>(config)
                    .unwrap()
                    .as_object()
                    .unwrap()
            );

            tear_down();
        }

        #[ignore]
        #[test]
        pub fn import_works_for_different_key_derivation_methods() {
            let ctx = setup();

            {
                let cmd = create_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "argon2i".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "argon2i".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            new_did(&ctx, SEED_MY1);
            use_did(&ctx, DID_MY1);

            let (_, path_str) = export_wallet_path();

            {
                let cmd = export_command::new();
                let mut params = CommandParams::new();
                params.insert("export_path", path_str.clone());
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert("key_derivation_method", "argon2m".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            let wallet_name = "imported_wallet";
            let key = "6nxtSiXFvBd593Y2DCed2dYvRY1PGK9WMtxCBjLzKgbw";
            // import wallet
            {
                let cmd = import_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", key.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            // open exported wallet
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", key.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            use_did(&ctx, DID_MY1);

            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }

            // delete first wallet
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", WALLET_KEY.to_string());
                params.insert("key_derivation_method", "argon2i".to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }

            // delete second wallet
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", key.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }

            tear_down();
        }
    }

    pub fn create_wallet(ctx: &CommandContext) {
        let create_cmd = create_command::new();
        let mut params = CommandParams::new();
        params.insert("name", WALLET.to_string());
        params.insert("key", WALLET_KEY_RAW.to_string());
        params.insert("key_derivation_method", "raw".to_string());
        create_cmd.execute(&ctx, &params).unwrap();
    }

    pub fn attach_wallet(ctx: &CommandContext) {
        let create_cmd = attach_command::new();
        let mut params = CommandParams::new();
        params.insert("name", WALLET.to_string());
        create_cmd.execute(&ctx, &params).unwrap();
    }

    pub fn create_and_open_wallet(ctx: &CommandContext) -> Rc<AnyStore> {
        {
            let create_cmd = create_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            create_cmd.execute(&ctx, &params).unwrap();
        }
        {
            let cmd = open_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            cmd.execute(&ctx, &params).unwrap();
        }

        ensure_opened_store(&ctx).unwrap()
    }

    pub fn open_wallet(ctx: &CommandContext) -> Rc<AnyStore> {
        {
            let cmd = open_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            cmd.execute(&ctx, &params).unwrap();
        }

        ensure_opened_store(&ctx).unwrap()
    }

    pub fn close_and_delete_wallet(ctx: &CommandContext) {
        {
            let cmd = close_command::new();
            let params = CommandParams::new();
            cmd.execute(&ctx, &params).unwrap();
        }

        {
            let cmd = delete_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            cmd.execute(&CommandContext::new(), &params).unwrap();
        }
    }

    pub fn close_wallet(ctx: &CommandContext) {
        {
            let cmd = close_command::new();
            let params = CommandParams::new();
            cmd.execute(&ctx, &params).unwrap();
        }
    }

    pub fn delete_wallet(ctx: &CommandContext) {
        {
            let cmd = delete_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            cmd.execute(&ctx, &params).unwrap();
        }
    }

    pub fn export_wallet_path() -> (PathBuf, String) {
        let path = EnvironmentUtils::tmp_file_path("export_file");
        (path.clone(), path.to_str().unwrap().to_string())
    }

    pub fn export_wallet(ctx: &CommandContext, path: &str) {
        let cmd = export_command::new();
        let mut params = CommandParams::new();
        params.insert("export_path", path.to_string());
        params.insert("export_key", EXPORT_KEY.to_string());
        cmd.execute(&ctx, &params).unwrap()
    }
}
