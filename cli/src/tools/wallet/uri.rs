/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    error::{CliError, CliResult},
    tools::wallet::Credentials,
    utils::environment::EnvironmentUtils,
};

use crate::tools::wallet::directory::WalletConfig;
use std::path::PathBuf;
use urlencoding::encode;

pub enum StorageType {
    Sqlite,
    Postgres,
}

impl StorageType {
    pub fn to_str(&self) -> &'static str {
        match self {
            StorageType::Sqlite => "sqlite",
            StorageType::Postgres => "postgres",
        }
    }
}

pub struct WalletUri(String);

impl WalletUri {
    pub fn value(&self) -> &str {
        self.0.as_str()
    }

    pub fn build(
        config: &WalletConfig,
        credentials: &Credentials,
        path: Option<&str>,
    ) -> CliResult<WalletUri> {
        let storage_type = Self::map_storage_type(&config.storage_type)?;
        let uri = match storage_type {
            StorageType::Sqlite => Self::build_sqlite_uri(config, credentials, path),
            StorageType::Postgres => Self::build_postgres_uri(config, credentials),
        }?;
        Ok(WalletUri(uri))
    }

    fn build_sqlite_uri(
        config: &WalletConfig,
        _credentials: &Credentials,
        path: Option<&str>,
    ) -> CliResult<String> {
        let mut path = match path {
            Some(path) => PathBuf::from(path),
            None => EnvironmentUtils::wallet_path(&config.id),
        };
        path.push(&config.id);
        path.set_extension("db");

        let uri = format!(
            "{}://{}",
            StorageType::Sqlite.to_str(),
            encode(&path.to_string_lossy())
        );

        Ok(uri)
    }

    fn build_postgres_uri(config: &WalletConfig, credentials: &Credentials) -> CliResult<String> {
        let storage_config = config
            .storage_config
            .as_ref()
            .ok_or(CliError::InvalidInput(
                "No 'storage_config' provided for postgres store".to_string(),
            ))?;
        let storage_credentials =
            credentials
                .storage_credentials
                .as_ref()
                .ok_or(CliError::InvalidInput(
                    "No 'storage_credentials' provided for postgres store".to_string(),
                ))?;

        let config_url = storage_config["url"]
            .as_str()
            .ok_or(CliError::InvalidInput(
                "No 'url' provided for postgres store".to_string(),
            ))?;

        let account = storage_credentials["account"]
            .as_str()
            .ok_or(CliError::InvalidInput(
                "No 'account' provided for postgres store".to_string(),
            ))?;

        let password = storage_credentials["password"]
            .as_str()
            .ok_or(CliError::InvalidInput(
                "No 'password' provided for postgres store".to_string(),
            ))?;

        let mut params: Vec<String> = Vec::new();
        if let Some(connection_timeout) = storage_config["connect_timeout"].as_u64() {
            params.push(format!("connect_timeout={}", connection_timeout))
        }
        if let Some(max_connections) = storage_config["max_connections"].as_u64() {
            params.push(format!("max_connections={}", max_connections))
        }
        if let Some(min_idle_count) = storage_config["min_idle_count"].as_u64() {
            params.push(format!("min_idle_count={}", min_idle_count))
        }
        if let Some(admin_account) = storage_credentials["admin_account"].as_str() {
            params.push(format!("admin_account={}", encode(admin_account)))
        }
        if let Some(admin_password) = storage_credentials["admin_password"].as_str() {
            params.push(format!("admin_password={}", encode(admin_password)))
        }
        let query_params = params.join("&").to_string();

        let uri = format!(
            "{}://{}:{}@{}/{}?{}",
            StorageType::Postgres.to_str(),
            encode(account),
            encode(password),
            config_url,
            encode(&config.id),
            &query_params
        );

        Ok(uri)
    }

    fn map_storage_type(storage_type: &str) -> CliResult<StorageType> {
        match storage_type {
            "default" | "sqlite" | "sqlite_storage" => Ok(StorageType::Sqlite),
            "postgres" | "postgres_storage" => Ok(StorageType::Postgres),
            value => Err(CliError::InvalidInput(format!(
                "Unsupported storage type provided: {}",
                value
            ))),
        }
    }
}
