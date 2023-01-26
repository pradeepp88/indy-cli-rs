/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
pub mod backup;
mod credentials;
pub mod libindy_backup_reader;
mod uri;
pub mod wallet_config;

use crate::{
    error::{CliError, CliResult},
    tools::did::constants::CATEGORY_DID,
    utils::futures::block_on,
};

use self::{
    credentials::WalletCredentials,
    uri::{StorageType, WalletUri},
};

use crate::tools::wallet::libindy_backup_reader::LibindyBackupRecord;
use crate::tools::{
    did::{constants::KEY_TYPE, DidInfo},
    wallet::{
        backup::BackupKind,
        libindy_backup_reader::{
            DidMetadataRecord, DidRecord, KeyRecord, LibindyBackupReader, TemporaryDidRecord,
        },
    },
};
use aries_askar::{
    any::AnyStore,
    kms::{KeyAlg, LocalKey},
    Entry, EntryTag, Error as AskarError, ErrorKind as AskarErrorKind, ManageBackend,
};
use backup::WalletBackup;
use serde_json::Value as JsonValue;
use wallet_config::{WalletConfig, WalletDirectory};

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
        block_on(async move {
            if config.exists() {
                return Err(CliError::Duplicate(format!(
                    "Wallet \"{}\" already exists",
                    config.id
                )));
            }

            let wallet_uri = WalletUri::build(config, credentials, None)?;
            let credentials = WalletCredentials::build(credentials)?;

            config.create_path()?;

            let store = wallet_uri
                .value()
                .provision_backend(
                    credentials.key_method,
                    credentials.key.as_ref(),
                    None,
                    false,
                )
                .await?;

            // Askar: If there is any opened store when delete the wallet, function returns ok and deletes wallet file successfully
            // But next if we create wallet with the same again it will contain old records
            // So we have to close all store handles
            store.close().await?;

            Ok(())
        })
    }

    pub fn open(config: &WalletConfig, credentials: &Credentials) -> CliResult<Wallet> {
        block_on(async move {
            let wallet_uri = WalletUri::build(config, credentials, None)?;
            let credentials = WalletCredentials::build(credentials)?;

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

    pub fn delete(config: &WalletConfig, credentials: &Credentials) -> CliResult<()> {
        block_on(async move {
            let wallet_uri = WalletUri::build(config, credentials, None)?;

            let removed = wallet_uri.value().remove_backend().await?;
            if !removed {
                return Err(CliError::InvalidEntityState(format!(
                    "Unable to delete wallet {}",
                    config.id
                )));
            }
            WalletDirectory::from_id(&config.id).delete()?;
            Ok(())
        })
    }

    pub fn list() -> Vec<JsonValue> {
        WalletDirectory::list_wallets()
    }

    pub fn export(&self, export_config: &ExportConfig) -> CliResult<()> {
        block_on(async move {
            let backup = WalletBackup::from_file(&export_config.path)?;

            let backup_config = WalletConfig {
                id: backup.id(),
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

            backup.init_dir()?;

            let backup_store = backup_uri
                .value()
                .provision_backend(
                    backup_credentials.key_method,
                    backup_credentials.key.as_ref(),
                    None,
                    false,
                )
                .await?;

            Self::copy_records_from_askar_store(&self.store, &backup_store).await?;

            backup_store.close().await?;

            Ok(())
        })
    }

    pub fn import(
        config: &WalletConfig,
        credentials: &Credentials,
        import_config: &ImportConfig,
    ) -> CliResult<()> {
        block_on(async move {
            let backup = WalletBackup::from_file(&import_config.path)?;
            if !backup.exists() {
                return Err(CliError::NotFound(format!(
                    "Wallet backup \"{}\" does not exist",
                    import_config.path
                )));
            }

            if config.exists() {
                return Err(CliError::Duplicate(format!(
                    "Wallet \"{}\" already exists",
                    config.id
                )));
            }

            match backup.kind()? {
                BackupKind::Askar => {
                    Self::import_askar_backup(&backup, &config, &credentials, &import_config).await
                }
                BackupKind::Libindy => {
                    Self::import_libindy_backup(&backup, &config, &credentials, &import_config)
                        .await
                }
            }
        })
    }

    async fn import_askar_backup(
        backup: &WalletBackup,
        config: &WalletConfig,
        credentials: &Credentials,
        import_config: &ImportConfig,
    ) -> CliResult<()> {
        // prepare config and credentials for backup and new wallet
        let backup_config = WalletConfig {
            id: backup.id(),
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

        // open backup storage
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

        // create directory for new wallet and provision it
        config.create_path()?;

        let new_store = new_wallet_uri
            .value()
            .provision_backend(
                new_wallet_credentials.key_method,
                new_wallet_credentials.key.as_ref(),
                None,
                false,
            )
            .await?;

        // copy all records from the backup into the new wallet
        Self::copy_records_from_askar_store(&backup_store, &new_store).await?;

        // finish
        backup_store.close().await?;
        new_store.close().await?;

        Ok(())
    }

    async fn import_libindy_backup(
        _backup: &WalletBackup,
        config: &WalletConfig,
        credentials: &Credentials,
        import_config: &ImportConfig,
    ) -> CliResult<()> {
        // prepare config and credentials for new wallet
        let new_wallet_uri = WalletUri::build(&config, &credentials, None)?;
        let new_wallet_credentials = WalletCredentials::build(&credentials)?;

        // init libindy backup reader
        let mut backup_reader = LibindyBackupReader::init(import_config)?;

        // create directory for new wallet and provision it
        config.create_path()?;

        let new_store = new_wallet_uri
            .value()
            .provision_backend(
                new_wallet_credentials.key_method,
                new_wallet_credentials.key.as_ref(),
                None,
                false,
            )
            .await?;

        // copy all records from the backup into the new wallet
        Self::copy_records_from_libindy_backup(&mut backup_reader, &new_store).await?;

        // finish
        new_store.close().await?;

        Ok(())
    }

    async fn copy_records_from_askar_store(from: &AnyStore, to: &AnyStore) -> CliResult<()> {
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

    async fn copy_records_from_libindy_backup(
        backup_reader: &mut LibindyBackupReader,
        to: &AnyStore,
    ) -> CliResult<()> {
        let mut to_session = to.session(None).await?;

        while let Some(record) = backup_reader.read_record()? {
            match record.type_.as_str() {
                KeyRecord::TYPE => {
                    let key_record = KeyRecord::from_str(&record.value)?;
                    let key = LocalKey::from_seed(KeyAlg::Ed25519, &key_record.key_bytes()?, None)?;

                    to_session
                        .insert_key(&record.id, &key, None, None, None)
                        .await
                        .ok();
                }
                DidRecord::TYPE => {
                    let did_record = DidRecord::from_str(&record.value)?;

                    let did_info = DidInfo {
                        did: did_record.did,
                        verkey: did_record.verkey,
                        verkey_type: KEY_TYPE.to_string(),
                        ..DidInfo::default()
                    };

                    to_session
                        .insert(
                            CATEGORY_DID,
                            &did_info.did,
                            &did_info.to_bytes()?,
                            Some(&did_info.tags()),
                            None,
                        )
                        .await
                        .ok();
                }
                TemporaryDidRecord::TYPE => {
                    let temporary_did_record = TemporaryDidRecord::from_str(&record.value)?;

                    let did_entry = to_session
                        .fetch(CATEGORY_DID, &temporary_did_record.did, true)
                        .await?
                        .ok_or_else(|| {
                            CliError::NotFound(format!(
                                "DID {} does not exits in the wallet.",
                                temporary_did_record.did
                            ))
                        })?;

                    let mut did_info: DidInfo = DidInfo::from_bytes(&did_entry.value)?;
                    did_info.next_verkey = Some(temporary_did_record.verkey.to_string());

                    to_session
                        .replace(
                            CATEGORY_DID,
                            &did_info.did,
                            &did_info.to_bytes()?,
                            Some(&did_entry.tags),
                            None,
                        )
                        .await
                        .ok();
                }
                DidMetadataRecord::TYPE => {
                    let did_metadata_record = DidMetadataRecord::from_str(&record.value)?;

                    let did_entry = to_session
                        .fetch(CATEGORY_DID, &record.id, true)
                        .await?
                        .ok_or_else(|| {
                            CliError::NotFound(format!(
                                "DID {} does not exits in the wallet.",
                                record.id
                            ))
                        })?;

                    let mut did_info: DidInfo = DidInfo::from_bytes(&did_entry.value)?;
                    did_info.metadata = Some(did_metadata_record.value);

                    to_session
                        .replace(
                            CATEGORY_DID,
                            &did_info.did,
                            &did_info.to_bytes()?,
                            Some(&did_entry.tags),
                            None,
                        )
                        .await
                        .ok();
                }
                _ => {
                    println_warn!("Unsupported record type {}", record.type_);
                    println_warn!("Record");
                    println_warn!("{:?}", record);
                }
            }
        }

        to_session.commit().await.map_err(CliError::from)
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
        session.commit().await.map_err(CliError::from)
    }

    pub async fn fetch_all_records(&self, category: &str) -> CliResult<Vec<Entry>> {
        let mut session = self.store.session(None).await?;
        session
            .fetch_all(category, None, None, false)
            .await
            .map_err(CliError::from)
    }

    pub async fn fetch_record(
        &self,
        category: &str,
        id: &str,
        for_update: bool,
    ) -> CliResult<Option<Entry>> {
        let mut session = self.store.session(None).await?;
        session
            .fetch(category, &id, for_update)
            .await
            .map_err(CliError::from)
    }

    pub async fn remove_record(&self, category: &str, id: &str) -> CliResult<()> {
        let mut session = self.store.session(None).await?;
        session.remove(category, id).await.map_err(CliError::from)?;
        session.commit().await.map_err(CliError::from)
    }

    pub async fn insert_key(
        &self,
        id: &str,
        key: &LocalKey,
        metadata: Option<&str>,
    ) -> CliResult<()> {
        let mut session = self.store.session(None).await?;
        session
            .insert_key(id, key, metadata, None, None)
            .await
            .map_err(CliError::from)
    }

    pub async fn fetch_key(&self, id: &str) -> CliResult<LocalKey> {
        let mut session = self.store.session(None).await?;
        session
            .fetch_key(id, false)
            .await?
            .ok_or_else(|| CliError::NotFound(format!("Key {} does not exits in the wallet!", id)))?
            .load_local_key()
            .map_err(CliError::from)
    }
}
