use crate::error::{CliError, CliResult};
use crate::utils::environment::EnvironmentUtils;
use crate::utils::wallet_config::{Config, WalletConfig};

use aries_askar::{
    Error as AskarError,
    ErrorKind as AskarErrorKind,
    any::AnyStore,
    future::block_on,
    ManageBackend,
    PassKey,
    StoreKeyMethod,
    Argon2Level,
    KdfMethod,
};

use serde_json::Value as JsonValue;
use std::fs;

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

struct AskarCredentials<'a> {
    key: PassKey<'a>,
    key_method: StoreKeyMethod,
    rekey: Option<PassKey<'a>>,
    rekey_method: Option<StoreKeyMethod>,
}

impl Wallet {
    pub fn create(config: &Config, credentials: &Credentials) -> CliResult<AnyStore> {
        if WalletConfig::exists(&config.id) {
            return Err(CliError::Duplicate(format!("Wallet \"{}\" already exists", config.id)));
        }

        Self::create_wallet_directory(config)?;

        let wallet_uri = Self::build_wallet_uri(config, credentials)?;
        let credentials1 = Self::build_credentials(credentials)?;

        block_on(async move {
            let store = wallet_uri
                .provision_backend(
                    credentials1.key_method,
                    credentials1.key.as_ref(),
                    None,
                    false,
                )
                .await
                .map_err(CliError::from)?;

            // Askar Error
            // If we have any opened store when later delete the wallet deletion will return ok
            // But next we can recreate wallet with the same same record
            store.close().await?;

            Ok(store)
        })
    }

    pub fn open(config: &Config, credentials: &Credentials) -> CliResult<AnyStore> {
        let wallet_uri = Self::build_wallet_uri(config, credentials)?;
        let credentials = Self::build_credentials(credentials)?;

        block_on(async move {
            let mut store: AnyStore = wallet_uri
                .open_backend(Some(credentials.key_method), credentials.key.as_ref(), None)
                .await
                .map_err(|err: AskarError| {
                    match err.kind() {
                        AskarErrorKind::NotFound => CliError::NotFound(format!("Wallet \"{}\" not found or unavailable.", config.id)),
                        _ => CliError::from(err)
                    }
                })?;

            if let (Some(rekey), Some(rekey_method)) = (credentials.rekey, credentials.rekey_method)
            {
                store.rekey(rekey_method, rekey).await?;
            }

            Ok(store)
        })
    }

    pub fn close(store: &AnyStore) -> CliResult<()> {
        block_on(async move { store.close().await.map_err(CliError::from) })
    }

    pub fn delete(config: &Config, credentials: &Credentials) -> CliResult<bool> {
        let wallet_uri = Self::build_wallet_uri(config, credentials)?;

        // Workaround to check that provided credentials are correct because Askar does not perform this check on delete call
        let store = Self::open(config, credentials)?;
        Self::close(&store)?;

        block_on(async move {
            let removed = wallet_uri.remove_backend().await.map_err(CliError::from)?;
            if !removed {
                return Err(CliError::InvalidEntityState(format!(
                    "Unable to delete wallet {}",
                    config.id
                )));
            }
            Self::delete_wallet_directory(config)?;
            Ok(removed)
        })
    }

    pub fn list() -> Vec<JsonValue> {
        WalletConfig::list()
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

    fn create_wallet_directory(config: &Config) -> CliResult<()> {
        let path = EnvironmentUtils::wallet_path(&config.id);
        fs::create_dir_all(path.as_path()).map_err(CliError::from)
    }

    fn delete_wallet_directory(config: &Config) -> CliResult<()> {
        let path = EnvironmentUtils::wallet_path(&config.id);
        if !path.exists() {
            return Err(CliError::NotFound(format!(
                "Wallet \"{}\" does not exist",
                config.id
            )));
        }
        fs::remove_dir_all(path.as_path()).map_err(CliError::from)
    }

    fn build_wallet_uri(config: &Config, credentials: &Credentials) -> CliResult<String> {
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

    fn build_credentials(credentials: &Credentials) -> CliResult<AskarCredentials> {
        let key_method = Self::map_key_derivation_method(
            credentials
                .key_derivation_method
                .as_ref()
                .map(String::as_str),
        )?;
        let key = PassKey::from(credentials.key.to_string());

        let rekey = credentials
            .rekey
            .as_ref()
            .map(|rekey| PassKey::from(rekey.to_string()));

        let rekey_method = match credentials.rekey {
            Some(_) => Some(Self::map_key_derivation_method(
                credentials
                    .rekey_derivation_method
                    .as_ref()
                    .map(String::as_str),
            )?),
            None => None,
        };

        Ok(AskarCredentials {
            key,
            key_method,
            rekey,
            rekey_method,
        })
    }

    fn map_storage_type(storage_type: &str) -> CliResult<StorageType> {
        match storage_type {
            "default" | "sqlite" => Ok(StorageType::Sqlite),
            "postgres" => Ok(StorageType::Postgres),
            value => Err(CliError::InvalidInput(format!(
                "Unsupported storage type provided: {}",
                value
            ))),
        }
    }

    fn map_key_derivation_method(key: Option<&str>) -> CliResult<StoreKeyMethod> {
        match key {
            None | Some("argon2m") => Ok(StoreKeyMethod::DeriveKey(KdfMethod::Argon2i(
                Argon2Level::Moderate,
            ))),
            Some("argon2i") => Ok(StoreKeyMethod::DeriveKey(KdfMethod::Argon2i(
                Argon2Level::Interactive,
            ))),
            Some("raw") => Ok(StoreKeyMethod::RawKey),
            Some(value) => Err(CliError::InvalidInput(format!(
                "Unsupported key derivation method \"{}\" provided for the wallet.",
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
