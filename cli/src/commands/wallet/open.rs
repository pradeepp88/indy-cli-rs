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
    tools::wallet::{Credentials, Wallet, wallet_config::WalletConfig},
    wallet::close_wallet,
};

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

        let id = ParamParser::get_str_param("name", params)?;
        let key = ParamParser::get_str_param("key", params)?;
        let rekey = ParamParser::get_opt_str_param("rekey", params)?;
        let key_derivation_method =
            ParamParser::get_opt_str_param("key_derivation_method", params)?;
        let rekey_derivation_method =
            ParamParser::get_opt_str_param("rekey_derivation_method", params)?;
        let storage_credentials = ParamParser::get_opt_object_param("storage_credentials", params)?;

        let config = WalletConfig::read(id)
            .map_err(|_| println_err!("Wallet \"{}\" isn't attached to CLI", id))?;

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            rekey: rekey.map(String::from),
            rekey_derivation_method: rekey_derivation_method.map(String::from),
            storage_credentials,
        };

        ctx.reset_active_did();

        if let Some(wallet) = ctx.get_opened_wallet() {
            if wallet.name == config.id {
                println_err!("Wallet \"{}\" already opened.", wallet.name);
                return Err(());
            }
        }
        if let Some(wallet) = ctx.take_opened_wallet()? {
            close_wallet(ctx, wallet)?;
        }

        let wallet = Wallet::open(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(&id))))?;

        ctx.set_opened_wallet(wallet);
        println_succ!("Wallet \"{}\" has been opened", id);

        trace!("execute << {:?}", ());
        Ok(())
    }

    pub fn cleanup(ctx: &CommandContext) {
        trace!("cleanup >> ctx {:?}", ctx);

        if let Ok(Some(wallet)) = ctx.take_opened_wallet() {
            close_wallet(ctx, wallet).ok();
        }

        trace!("cleanup <<");
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, setup_with_wallet, tear_down, tear_down_with_wallet};

    mod open {
        use super::*;
        use crate::wallet::tests::{
            close_and_delete_wallet, create_wallet, delete_wallet, WALLET, WALLET_KEY,
            WALLET_KEY_RAW,
        };

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
            ctx.ensure_opened_wallet().unwrap();
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
                cmd.execute(&ctx, &params).unwrap_err();
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
}
