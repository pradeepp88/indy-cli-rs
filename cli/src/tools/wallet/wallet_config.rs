/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::utils::environment::EnvironmentUtils;

use crate::error::{CliError, CliResult};
use serde_json::Value as JsonValue;
use std::{
    fs,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WalletConfig {
    pub id: String,
    pub storage_type: String,
    pub storage_config: Option<JsonValue>,
}

impl WalletConfig {
    pub(crate) fn store(&self) -> CliResult<()> {
        Self::create_wallets_directory()?;

        let mut config_file = File::create(&self.path())?;
        let config_json = json!(self).to_string();
        config_file.write_all(config_json.as_bytes())?;
        config_file.sync_all()?;
        Ok(())
    }

    pub(crate) fn read(id: &str) -> CliResult<Self> {
        let path = EnvironmentUtils::wallet_config_path(id);

        let mut config_json = String::new();
        let mut file = File::open(path)?;
        file.read_to_string(&mut config_json)?;

        serde_json::from_str(&config_json).map_err(CliError::from)
    }

    pub(crate) fn delete(&self) -> CliResult<()> {
        fs::remove_file(&self.path()).map_err(CliError::from)
    }

    pub(crate) fn exists(&self) -> bool {
        self.path().exists()
    }

    pub(crate) fn create_path(&self) -> CliResult<()> {
        WalletDirectory::from_id(&self.id).create()
    }

    fn path(&self) -> PathBuf {
        EnvironmentUtils::wallet_config_path(&self.id)
    }

    pub(crate) fn create_wallets_directory() -> CliResult<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .create(EnvironmentUtils::wallets_path())
            .map_err(CliError::from)
    }
}

pub struct WalletDirectory {
    id: String,
    path: PathBuf,
}

impl WalletDirectory {
    pub(crate) fn from_id(id: &str) -> WalletDirectory {
        let path = EnvironmentUtils::wallet_path(id);
        WalletDirectory {
            id: id.to_string(),
            path,
        }
    }

    pub(crate) fn create(&self) -> CliResult<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .create(&self.path)
            .map_err(CliError::from)
    }

    pub(crate) fn delete(&self) -> CliResult<()> {
        if !self.path.exists() {
            return Err(CliError::NotFound(format!(
                "Wallet \"{}\" does not exist",
                self.id
            )));
        }
        fs::remove_dir_all(self.path.as_path()).map_err(CliError::from)
    }

    pub(crate) fn list_wallets() -> Vec<JsonValue> {
        let mut configs: Vec<JsonValue> = Vec::new();

        if let Ok(entries) = fs::read_dir(EnvironmentUtils::wallets_path()) {
            for entry in entries {
                let file = if let Ok(dir_entry) = entry {
                    dir_entry
                } else {
                    continue;
                };

                let mut config_json = String::new();

                File::open(file.path())
                    .ok()
                    .and_then(|mut f| f.read_to_string(&mut config_json).ok())
                    .and_then(|_| serde_json::from_str::<JsonValue>(config_json.as_str()).ok())
                    .map(|config| configs.push(config));
            }
        }

        configs
    }
}
