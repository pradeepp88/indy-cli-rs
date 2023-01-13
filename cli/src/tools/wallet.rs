use crate::error::{CliError, CliResult};
use crate::utils::environment::EnvironmentUtils;
use crate::utils::wallet_config::Config;

use aries_askar::{any::AnyStore, future::block_on, ManageBackend, PassKey, StoreKeyMethod};
use serde_json::Value as JsonValue;
use std::{fs, fs::File, io::Read};

pub struct Wallet {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub key: String,
    pub key_derivation_method: Option<String>,
    pub rekey: Option<String>,
    pub rekey_derivation_method: Option<String>,
    pub storage_credentials: Option<JsonValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportConfig {
    pub path: String,
    pub key: String,
    pub key_derivation_method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportConfig {
    pub path: String,
    pub key: String,
}

impl Wallet {
    pub fn create(config: &Config, credentials: &Credentials) -> CliResult<AnyStore> {
        Self::init_wallet_directory(config)?;
        let wallet_uri = Self::build_uri(config, credentials)?;
        let (method, key) = Self::build_credentials(credentials)?;

        block_on(async move {
            wallet_uri
                .provision_backend(method, key.as_ref(), None, false)
                .await
                .map_err(CliError::from)
        })
    }

    pub fn open(config: &Config, credentials: &Credentials) -> CliResult<AnyStore> {
        let wallet_uri = Self::build_uri(config, credentials)?;
        let (method, key) = Self::build_credentials(credentials)?;

        block_on(async move {
            wallet_uri
                .open_backend(Some(method), key.as_ref(), None)
                .await
                .map_err(CliError::from)
        })
    }

    pub fn delete(config: &Config, credentials: &Credentials) -> CliResult<bool> {
        let wallet_uri = Self::build_uri(config, credentials)?;

        block_on(async move { wallet_uri.remove_backend().await.map_err(CliError::from) })
    }

    pub fn close(store: &AnyStore) -> CliResult<()> {
        block_on(async move { store.close().await.map_err(CliError::from) })
    }

    pub fn list() -> Vec<JsonValue> {
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

    pub fn export(_store: &AnyStore, _export_config: &ExportConfig) -> CliResult<()> {
        unimplemented!()
        // wallet::export_wallet(wallet_handle, export_config_json).wait()
    }

    pub fn import(
        _config: &Config,
        _credentials: &Credentials,
        _import_config: &ImportConfig,
    ) -> CliResult<()> {
        unimplemented!()
        // wallet::import_wallet(config, credentials, import_config_json).wait()
    }

    fn init_wallet_directory(config: &Config) -> CliResult<()> {
        let path = EnvironmentUtils::wallet_path(&config.id);
        fs::create_dir_all(path.as_path()).map_err(CliError::from)
    }

    fn build_uri(config: &Config, credentials: &Credentials) -> CliResult<String> {
        let storage_type = Self::map_storage_type(&config.storage_type)?;
        match storage_type {
            StorageType::Sqlite => Self::build_sqlite_uri(config, credentials),
            StorageType::Postgres => Self::build_postgres_uri(config, credentials),
        }
    }

    fn build_sqlite_uri(config: &Config, _credentials: &Credentials) -> CliResult<String> {
        let mut path = EnvironmentUtils::wallet_path(&config.id);
        path.push(&config.id);
        path.set_extension("db");
        let uri = format!("{}://{}", "sqlite", path.to_string_lossy());
        Ok(uri)
    }

    fn build_postgres_uri(config: &Config, credentials: &Credentials) -> CliResult<String> {
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

        // FIXME: Find proper way to build and encode URI
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
            params.push(format!("admin_account={}", admin_account))
        }
        if let Some(admin_password) = storage_credentials["admin_password"].as_str() {
            params.push(format!("admin_password={}", admin_password))
        }
        let query_params = params.join("&").to_string();

        let uri = format!(
            "{}:{}@{}/{}?{}",
            account, password, config_url, &config.id, query_params
        );

        Ok(uri)
    }

    fn build_credentials(credentials: &Credentials) -> CliResult<(StoreKeyMethod, PassKey)> {
        let method = Self::map_key_derivation_method(
            credentials
                .key_derivation_method
                .as_ref()
                .map(String::as_str),
        )?;
        let key = PassKey::from(credentials.key.to_string());
        Ok((method, key))
    }

    fn map_storage_type(storage_type: &str) -> CliResult<StorageType> {
        match storage_type {
            "default" | "sqlite" => Ok(StorageType::Sqlite),
            "postgres" => Ok(StorageType::Postgres),
            value => Err(CliError::InvalidInput(format!(
                "Unsupported storage type {} provided",
                value
            ))),
        }
    }

    fn map_key_derivation_method(key: Option<&str>) -> CliResult<StoreKeyMethod> {
        match key {
            None | Some("argon2m") => Ok(StoreKeyMethod::Unprotected),
            Some("argon2i") => Ok(StoreKeyMethod::Unprotected),
            Some("raw") => Ok(StoreKeyMethod::RawKey),
            Some(value) => Err(CliError::InvalidInput(format!(
                "Unsupported key derivation method provided {}",
                value
            ))),
        }
    }
}

enum StorageType {
    Sqlite,
    Postgres,
}

// impl StorageType {
//     fn to_str(&self) -> &'static str {
//         match self {
//             StorageType::Sqlite => "sqlite",
//             StorageType::Postgres => "postgres",
//         }
//     }
// }
