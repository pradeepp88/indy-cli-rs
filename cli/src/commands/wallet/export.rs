/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
};

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

        let wallet = ctx.ensure_opened_wallet()?;

        let export_path = ParamParser::get_str_param("export_path", params)?;
        let export_key = ParamParser::get_str_param("export_key", params)?;
        let export_key_derivation_method =
            ParamParser::get_opt_str_param("export_key_derivation_method", params)?;

        let export_config = ExportConfig {
            path: export_path.to_string(),
            key: export_key.to_string(),
            key_derivation_method: export_key_derivation_method.map(String::from),
        };

        trace!(
            "Wallet::export_wallet try: wallet_name {}, export_path {}",
            wallet.name,
            export_path
        );

        wallet
            .export(&export_config)
            .map_err(|err| println_err!("{}", err.message(Some(&wallet.name))))?;

        println_succ!(
            "Wallet \"{}\" has been exported to the file \"{}\"",
            wallet.name,
            export_path
        );

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup_with_wallet, tear_down_with_wallet};

    mod export {
        use super::*;
        use crate::wallet::tests::{
            export_wallet, export_wallet_path, EXPORT_KEY, EXPORT_KEY_DERIVATION_METHOD,
        };

        #[test]
        pub fn export_works() {
            let ctx = setup_with_wallet();

            let (path, path_str) = export_wallet_path();
            {
                let cmd = export_command::new();
                let mut params = CommandParams::new();
                params.insert("export_path", path_str);
                params.insert("export_key", EXPORT_KEY.to_string());
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }

            assert!(path.exists());
            tear_down_with_wallet(&ctx);
        }

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
                params.insert(
                    "export_key_derivation_method",
                    EXPORT_KEY_DERIVATION_METHOD.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }
    }
}
