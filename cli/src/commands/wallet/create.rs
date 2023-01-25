/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::wallet::{wallet_config::WalletConfig, Credentials, Wallet},
};

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

        let id = ParamParser::get_str_param("name", params)?;
        let key = ParamParser::get_str_param("key", params)?;
        let key_derivation_method =
            ParamParser::get_opt_str_param("key_derivation_method", params)?;
        let storage_type =
            ParamParser::get_opt_str_param("storage_type", params)?.unwrap_or("default");
        let storage_config = ParamParser::get_opt_object_param("storage_config", params)?;
        let storage_credentials = ParamParser::get_opt_object_param("storage_credentials", params)?;

        let config = WalletConfig {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };
        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            storage_credentials,
            ..Credentials::default()
        };

        trace!("Wallet::create_wallet try: config {:?}", config);

        Wallet::create(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(&id))))?;

        config
            .store()
            .map_err(|err| println_err!("Cannot store wallet \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been created", id);

        trace!("execute << {:?}", ());
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, setup_with_wallet, tear_down, tear_down_with_wallet};

    mod create {
        use super::*;
        use crate::wallet::{
            delete_command,
            tests::{delete_wallet, WALLET, WALLET_KEY, WALLET_KEY_RAW},
        };

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
}
