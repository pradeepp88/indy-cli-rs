/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
pub mod backup;
mod credentials;
pub mod directory;
mod uri;

use crate::{
    error::{CliError, CliResult},
    tools::did::constants::CATEGORY_DID,
    utils::futures::block_on,
};

use self::{
    credentials::WalletCredentials,
    uri::{StorageType, WalletUri},
};

use crate::tools::did::key::Key;
use aries_askar::{
    any::AnyStore, kms::LocalKey, Entry, EntryTag, Error as AskarError,
    ErrorKind as AskarErrorKind, ManageBackend,
};
use backup::WalletBackup;
use directory::{WalletConfig, WalletDirectory};
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub struct Wallet {
    pub name: String,
    pub store: AnyStore,
}

#[derive(Debug, Default, Serialize, Deserialize)]
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
    pub key_derivation_method: Option<String>,
}

impl Wallet {
    pub fn create(config: &WalletConfig, credentials: &Credentials) -> CliResult<()> {
        if WalletDirectory::is_wallet_config_exist(&config.id) {
            return Err(CliError::Duplicate(format!(
                "Wallet \"{}\" already exists",
                config.id
            )));
        }

        let wallet_uri = WalletUri::build(config, credentials, None)?;
        let credentials = WalletCredentials::build(credentials)?;

        WalletDirectory::create(config)?;

        block_on(async move {
            let store = wallet_uri
                .value()
                .provision_backend(
                    credentials.key_method,
                    credentials.key.as_ref(),
                    None,
                    false,
                )
                .await
                .map_err(CliError::from)?;

            // Askar: If there is any opened store when delete the wallet, function returns ok and deletes wallet file successfully
            // But next if we create wallet with the same again it will contain old records
            // So we have to close all store handles
            store.close().await?;

            Ok(())
        })
    }

    pub fn open(config: &WalletConfig, credentials: &Credentials) -> CliResult<Wallet> {
        let wallet_uri = WalletUri::build(config, credentials, None)?;
        let credentials = WalletCredentials::build(credentials)?;

        block_on(async move {
            let mut store: AnyStore = wallet_uri
                .value()
                .open_backend(Some(credentials.key_method), credentials.key.as_ref(), None)
                .await
                .map_err(|err: AskarError| match err.kind() {
                    AskarErrorKind::NotFound => CliError::NotFound(format!(
                        "Wallet \"{}\" not found or unavailable.",
                        config.id
                    )),
                    _ => CliError::from(err),
                })?;

            if let (Some(rekey), Some(rekey_method)) = (credentials.rekey, credentials.rekey_method)
            {
                store.rekey(rekey_method, rekey).await?;
            }

            Ok(Wallet {
                store,
                name: config.id.to_string(),
            })
        })
    }

    pub fn close(self) -> CliResult<()> {
        block_on(async move { self.store.close().await.map_err(CliError::from) })
    }

    pub fn delete(config: &WalletConfig, credentials: &Credentials) -> CliResult<bool> {
        let wallet_uri = WalletUri::build(config, credentials, None)?;

        block_on(async move {
            let removed = wallet_uri
                .value()
                .remove_backend()
                .await
                .map_err(CliError::from)?;
            if !removed {
                return Err(CliError::InvalidEntityState(format!(
                    "Unable to delete wallet {}",
                    config.id
                )));
            }
            WalletDirectory::delete(&config)?;
            Ok(removed)
        })
    }

    pub fn list() -> Vec<JsonValue> {
        WalletDirectory::list_wallets()
    }

    pub fn export(&self, export_config: &ExportConfig) -> CliResult<()> {
        let backup_config = WalletConfig {
            id: WalletBackup::get_id(&export_config.path),
            storage_type: StorageType::Sqlite.to_str().to_string(),
            ..WalletConfig::default()
        };
        let backup_credentials = Credentials {
            key: export_config.key.clone(),
            key_derivation_method: export_config.key_derivation_method.clone(),
            ..Credentials::default()
        };

        let backup_uri = WalletUri::build(
            &backup_config,
            &backup_credentials,
            Some(&export_config.path),
        )?;
        let backup_credentials = WalletCredentials::build(&backup_credentials)?;

        WalletBackup::init_directory(&export_config.path)?;

        block_on(async move {
            let backup_store = backup_uri
                .value()
                .provision_backend(
                    backup_credentials.key_method,
                    backup_credentials.key.as_ref(),
                    None,
                    false,
                )
                .await
                .map_err(CliError::from)?;

            Self::copy_records(&self.store, &backup_store).await?;

            backup_store.close().await?;

            Ok(())
        })
    }

    pub fn import(
        config: &WalletConfig,
        credentials: &Credentials,
        import_config: &ImportConfig,
    ) -> CliResult<()> {
        if !WalletBackup::is_wallet_backup_exist(&import_config.path) {
            return Err(CliError::NotFound(format!(
                "Wallet backup \"{}\" not found",
                import_config.path
            )));
        }

        if WalletDirectory::is_wallet_config_exist(&config.id) {
            return Err(CliError::Duplicate(format!(
                "Wallet \"{}\" already exists",
                config.id
            )));
        }

        let backup_config = WalletConfig {
            id: WalletBackup::get_id(&import_config.path),
            storage_type: StorageType::Sqlite.to_str().to_string(),
            ..WalletConfig::default()
        };
        let backup_credentials = Credentials {
            key: import_config.key.clone(),
            key_derivation_method: import_config.key_derivation_method.clone(),
            ..Credentials::default()
        };

        let backup_wallet_uri = WalletUri::build(
            &backup_config,
            &backup_credentials,
            Some(&import_config.path),
        )?;
        let backup_wallet_credentials = WalletCredentials::build(&backup_credentials)?;

        let new_wallet_uri = WalletUri::build(&config, &credentials, None)?;
        let new_wallet_credentials = WalletCredentials::build(&credentials)?;

        block_on(async move {
            let backup_store: AnyStore = backup_wallet_uri
                .value()
                .open_backend(
                    Some(backup_wallet_credentials.key_method),
                    backup_wallet_credentials.key.as_ref(),
                    None,
                )
                .await
                .map_err(|err: AskarError| match err.kind() {
                    AskarErrorKind::NotFound => CliError::NotFound(err.to_string()),
                    _ => CliError::from(err),
                })?;

            WalletDirectory::create(config)?;

            let new_store = new_wallet_uri
                .value()
                .provision_backend(
                    new_wallet_credentials.key_method,
                    new_wallet_credentials.key.as_ref(),
                    None,
                    false,
                )
                .await
                .map_err(CliError::from)?;

            Self::copy_records(&backup_store, &new_store).await?;

            backup_store.close().await?;
            new_store.close().await?;

            Ok(())
        })
    }

    async fn copy_records(from: &AnyStore, to: &AnyStore) -> CliResult<()> {
        let mut from_session = from.session(None).await?;
        let mut to_session = to.session(None).await?;

        let did_entries = from_session
            .fetch_all(CATEGORY_DID, None, None, false)
            .await?;

        for entry in did_entries {
            to_session
                .insert(
                    &entry.category,
                    &entry.name,
                    &entry.value,
                    Some(&entry.tags),
                    None,
                )
                .await
                .ok();
        }

        let key_entries = from_session
            .fetch_all_keys(None, None, None, None, false)
            .await?;

        for entry in key_entries {
            to_session
                .insert_key(
                    entry.name(),
                    &entry.load_local_key()?,
                    entry.metadata(),
                    None,
                    None,
                )
                .await
                .ok();
        }

        to_session.commit().await?;
        from_session.commit().await?;

        Ok(())
    }

    pub async fn store_record(
        &self,
        category: &str,
        id: &str,
        value: &[u8],
        tags: Option<&[EntryTag]>,
        new: bool,
    ) -> CliResult<()> {
        let mut session = self.store.session(None).await?;
        if new {
            session.insert(category, id, value, tags, None).await?
        } else {
            session.replace(category, id, value, tags, None).await?
        }
        session.commit().await?;
        Ok(())
    }

    pub async fn fetch_all_record(&self, category: &str) -> CliResult<Vec<Entry>> {
        let mut session = self.store.session(None).await?;
        let records = session.fetch_all(category, None, None, false).await?;
        session.commit().await?;
        Ok(records)
    }

    pub async fn fetch_record(
        &self,
        category: &str,
        id: &str,
        for_update: bool,
    ) -> CliResult<Option<Entry>> {
        let mut session = self.store.session(None).await?;
        let record = session.fetch(category, &id, for_update).await?;
        session.commit().await?;
        Ok(record)
    }

    pub async fn remove_record(&self, category: &str, id: &str) -> CliResult<()> {
        let mut session = self.store.session(None).await?;
        session.remove(category, id).await.map_err(CliError::from)?;
        session.commit().await?;
        Ok(())
    }

    pub async fn insert_key(&self, id: &str, key: &Key, metadata: Option<&str>) -> CliResult<()> {
        let mut session = self.store.session(None).await?;
        session
            .insert_key(id, key.value(), metadata, None, None)
            .await?;
        session.commit().await?;
        Ok(())
    }

    pub async fn fetch_key(&self, id: &str) -> CliResult<LocalKey> {
        let mut session = self.store.session(None).await?;
        let key = session
            .fetch_key(id, false)
            .await?
            .ok_or_else(|| CliError::NotFound(format!("Key {} does not exits in the wallet!", id)))?
            .load_local_key()?;
        session.commit().await?;
        Ok(key)
    }
}
