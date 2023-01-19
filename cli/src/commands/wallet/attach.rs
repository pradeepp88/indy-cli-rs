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
    tools::wallet::directory::{WalletConfig, WalletDirectory},
};

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

        let id = ParamParser::get_str_param("name", params)?;
        let storage_type =
            ParamParser::get_opt_str_param("storage_type", params)?.unwrap_or("default");
        let storage_config = ParamParser::get_opt_object_param("storage_config", params)?;

        if WalletDirectory::is_wallet_config_exist(id) {
            println_err!("Wallet \"{}\" is already attached to CLI", id);
            return Err(());
        }

        let config = WalletConfig {
            id: id.to_string(),
            storage_type: storage_type.to_string(),
            storage_config,
        };

        WalletDirectory::store_wallet_config(id, &config)
            .map_err(|err| println_err!("Cannot store wallet \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been attached", id);

        trace!("execute << ");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod attach {
        use super::*;
        use crate::{
            tools::wallet::Wallet,
            wallet::tests::{attach_wallet, WALLET},
        };

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
}
