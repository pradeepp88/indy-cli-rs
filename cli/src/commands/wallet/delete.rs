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
    tools::wallet::{directory::WalletDirectory, Credentials, Wallet},
    wallet::close_wallet,
};

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

        let id = ParamParser::get_str_param("name", params)?;
        let key = ParamParser::get_str_param("key", params)?;
        let key_derivation_method =
            ParamParser::get_opt_str_param("key_derivation_method", params)?;
        let storage_credentials = ParamParser::get_opt_object_param("storage_credentials", params)?;

        let config = WalletDirectory::read_wallet_config(id)
            .map_err(|_| println_err!("Wallet \"{}\" isn't attached to CLI", id))?;

        let credentials = Credentials {
            key: key.to_string(),
            key_derivation_method: key_derivation_method.map(String::from),
            storage_credentials,
            ..Credentials::default()
        };

        if let Some(wallet) = ctx.take_opened_wallet()? {
            close_wallet(ctx, wallet)?;
        }

        Wallet::delete(&config, &credentials)
            .map_err(|err| println_err!("{}", err.message(Some(id))))?;

        WalletDirectory::delete_wallet_config(id)
            .map_err(|err| println_err!("Cannot delete \"{}\" config file: {:?}", id, err))?;

        println_succ!("Wallet \"{}\" has been deleted", id);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup, tear_down};

    mod delete {
        use super::*;
        use crate::wallet::tests::{
            create_and_open_wallet, create_wallet, WALLET, WALLET_KEY, WALLET_KEY_RAW,
        };

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

        #[test]
        pub fn delete_works_for_wrong_key() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = delete_command::new();
                let mut params = CommandParams::new();
                params.insert("name", WALLET.to_string());
                params.insert("key", "other_key".to_string());
                cmd.execute(&ctx, &params).unwrap(); // Askar does not check credentials!
            }
            tear_down();
        }
    }
}
