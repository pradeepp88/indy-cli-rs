/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{CommandGroup, CommandGroupMetadata},
    tools::wallet::Wallet,
};

pub mod attach;
pub mod close;
pub mod create;
pub mod delete;
pub mod detach;
pub mod export;
pub mod import;
pub mod list;
pub mod open;

pub use self::{
    attach::*, close::*, create::*, delete::*, detach::*, export::*, import::*, list::*, open::*,
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "wallet",
        "Wallet management commands"
    ));
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
    use crate::{
        command_executor::{CommandContext, CommandParams},
        utils::environment::EnvironmentUtils,
    };
    use std::{path::PathBuf, rc::Rc};

    pub const WALLET: &str = "wallet";
    pub const WALLET_KEY: &str = "wallet_key";
    pub const WALLET_KEY_RAW: &str = "6nxtSiXFvBd593Y2DCed2dYvRY1PGK9WMtxCBjLzKgbw";
    pub const EXPORT_KEY: &str = "6nxtSiXFvBd593Y2DCed2dYvRY1PGK9WMtxCBjLzKgex";
    pub const EXPORT_KEY_DERIVATION_METHOD: &str = "raw";

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

    pub fn create_and_open_wallet(ctx: &CommandContext) -> Rc<Wallet> {
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

        ctx.ensure_opened_wallet().unwrap()
    }

    pub fn open_wallet(ctx: &CommandContext) -> Rc<Wallet> {
        {
            let cmd = open_command::new();
            let mut params = CommandParams::new();
            params.insert("name", WALLET.to_string());
            params.insert("key", WALLET_KEY_RAW.to_string());
            params.insert("key_derivation_method", "raw".to_string());
            cmd.execute(&ctx, &params).unwrap();
        }

        ctx.ensure_opened_wallet().unwrap()
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
        params.insert(
            "export_key_derivation_method",
            EXPORT_KEY_DERIVATION_METHOD.to_string(),
        );
        cmd.execute(&ctx, &params).unwrap()
    }
}
