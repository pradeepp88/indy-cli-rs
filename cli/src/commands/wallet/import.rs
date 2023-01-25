/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{
        Command, CommandContext, CommandMetadata, CommandParams, DynamicCompletionType,
    },
    params_parser::ParamParser,
    tools::wallet::{wallet_config::WalletConfig, Credentials, Wallet},
};

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
                .add_required_deferred_param("export_key_derivation_method", "Algorithm to use for export key derivation")
                .add_example("wallet import wallet1 key export_path=/home/indy/export_wallet export_key")
                .add_example(r#"wallet import wallet1 key export_path=/home/indy/export_wallet export_key storage_type=default storage_config={"key1":"value1","key2":"value2"}"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let id = ParamParser::get_str_param("name", params)?;
        let key = ParamParser::get_str_param("key", params)?;
        let key_derivation_method =
            ParamParser::get_opt_str_param("key_derivation_method", params)?;
        let export_path = ParamParser::get_str_param("export_path", params)?;
        let export_key = ParamParser::get_str_param("export_key", params)?;
        let export_key_derivation_method =
            ParamParser::get_opt_str_param("export_key_derivation_method", params)?;
        let storage_type =
            ParamParser::get_opt_str_param("storage_type", params)?.unwrap_or("default");
        let storage_config = ParamParser::get_opt_object_param("storage_config", params)?;
        let storage_credentials = ParamParser::get_opt_object_param("storage_credentials", params)?;

        let config = WalletConfig {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };

        let import_config = ImportConfig {
            path: export_path.to_string(),
            key: export_key.to_string(),
            key_derivation_method: export_key_derivation_method.map(String::from),
        };

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: None,
            rekey_derivation_method: None,
            storage_credentials,
        };

        if config.exists() {
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

        config
            .store()
            .map_err(|err| println_err!("Cannot store \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been created", id);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod import {
        use super::*;
        use crate::{
            commands::setup_with_wallet,
            did::tests::{new_did, use_did, DID_MY1, SEED_MY1},
            wallet::{
                close_command, create_command, delete_command, export_command, open_command,
                tests::{
                    close_and_delete_wallet, create_and_open_wallet, export_wallet,
                    export_wallet_path, EXPORT_KEY, EXPORT_KEY_DERIVATION_METHOD, WALLET,
                    WALLET_KEY, WALLET_KEY_RAW,
                },
            },
        };

        #[test]
        pub fn import_works() {
            let ctx = setup_with_wallet();

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
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }

            // open exported wallet
            {
                let cmd = open_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            use_did(&ctx, DID_MY1);

            close_and_delete_wallet(&ctx);

            // delete first wallet
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", wallet_name.to_string());
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                cmd.execute(&CommandContext::new(), &params).unwrap();
            }

            tear_down();
        }

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
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

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
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }

            close_and_delete_wallet(&ctx);
            tear_down();
        }

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
                params.insert("key", WALLET_KEY_RAW.to_string());
                params.insert("key_derivation_method", "raw".to_string());
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
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
                params.insert("export_key_derivation_method", "argon2i".to_string());
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
                params.insert("export_key_derivation_method", "argon2i".to_string());
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
}
