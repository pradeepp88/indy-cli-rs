/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
pub mod constants;
pub mod key;
pub mod seed;

use crate::{
    error::{CliError, CliResult},
    utils::futures::block_on,
};

use crate::tools::wallet::Wallet;
use aries_askar::{Entry, EntryTag};
use indy_utils::{base58, did::DidValue, keys::EncodedVerKey, Qualifiable};

use self::{
    constants::{CATEGORY_DID, KEY_TYPE},
    key::Key,
};

pub struct Did {}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DidInfo {
    pub did: String,
    pub verkey: String,
    pub verkey_type: String,
    pub method: Option<String>,
    pub metadata: Option<String>,
    pub next_verkey: Option<String>,
}

impl Did {
    pub fn create(
        store: &Wallet,
        did: Option<&str>,
        seed: Option<&str>,
        metadata: Option<&str>,
        method: Option<&str>,
    ) -> CliResult<(String, String)> {
        block_on(async move {
            let key = Key::create(store, seed, metadata).await?;

            let verkey = key.verkey()?;
            let verkey_bytes = key.verkey_bytes()?;
            let mut did = match did {
                Some(did) => did.to_string(),
                None => base58::encode(&verkey_bytes[0..16]),
            };

            let existing_did = Self::get_opt_record(store, &did, false).await?;
            if existing_did.is_some() {
                return Err(CliError::Duplicate(format!(
                    "DID already exits in the wallet"
                )));
            }

            let mut tags = vec![
                EntryTag::Encrypted("verkey".to_string(), verkey.to_string()),
                EntryTag::Encrypted("verkey_type".to_string(), KEY_TYPE.to_string()),
            ];
            if let Some(method) = method {
                did = DidValue(did.to_string()).to_qualified(method)?.to_string();
                tags.push(EntryTag::Encrypted(
                    "method".to_string(),
                    method.to_string(),
                ))
            }

            let did_info = DidInfo {
                did: did.clone(),
                verkey: verkey.clone(),
                verkey_type: KEY_TYPE.to_string(),
                method: method.map(String::from),
                metadata: metadata.map(String::from),
                next_verkey: None,
            };

            let value = serde_json::to_vec(&did_info)?;
            store
                .store_record(CATEGORY_DID, &did_info.did, &value, Some(&tags), true)
                .await?;

            Ok((did, verkey))
        })
    }

    pub fn replace_keys_start(store: &Wallet, did: &str, seed: Option<&str>) -> CliResult<String> {
        block_on(async move {
            let (did_entry, mut did_info) = Self::get_record(store, &did, true).await?;

            let key = Key::create(store, seed, None).await?;
            let verkey = key.verkey()?;

            did_info.next_verkey = Some(verkey.clone());

            let value = serde_json::to_vec(&did_info)?;
            store
                .store_record(
                    CATEGORY_DID,
                    &did_info.did,
                    &value,
                    Some(&did_entry.tags),
                    false,
                )
                .await?;

            Ok(verkey)
        })
    }

    pub fn replace_keys_apply(store: &Wallet, did: &str) -> CliResult<()> {
        block_on(async move {
            let (did_entry, mut did_info) = Self::get_record(store, &did, true).await?;

            let next_verkey = did_info.next_verkey.ok_or_else(|| {
                CliError::InvalidEntityState(format!("Next key is not set for the DID {}.", did))
            })?;

            did_info.verkey = next_verkey;
            did_info.next_verkey = None;

            let value = serde_json::to_vec(&did_info)?;
            store
                .store_record(
                    CATEGORY_DID,
                    &did_info.did,
                    &value,
                    Some(&did_entry.tags),
                    false,
                )
                .await?;

            Ok(())
        })
    }

    pub fn set_metadata(store: &Wallet, did: &str, metadata: &str) -> CliResult<()> {
        block_on(async move {
            let (did_entry, mut did_info) = Self::get_record(store, &did, true).await?;

            did_info.metadata = Some(metadata.to_string());

            let value = serde_json::to_vec(&did_info)?;
            store
                .store_record(
                    CATEGORY_DID,
                    &did_info.did,
                    &value,
                    Some(&did_entry.tags),
                    false,
                )
                .await?;

            Ok(())
        })
    }

    pub fn get(store: &Wallet, did: &DidValue) -> CliResult<DidInfo> {
        block_on(async move {
            let (_, did_info) = Self::get_record(store, &did, true).await?;
            Ok(did_info)
        })
    }

    pub fn list(store: &Wallet) -> CliResult<Vec<DidInfo>> {
        block_on(async move {
            store
                .fetch_all_records(CATEGORY_DID)
                .await?
                .iter()
                .map(|did| serde_json::from_slice(&did.value).map_err(CliError::from))
                .collect::<CliResult<Vec<DidInfo>>>()
        })
    }

    pub fn abbreviate_verkey(did: &str, verkey: &str) -> CliResult<String> {
        let did = DidValue(did.to_string()).to_short().to_string();
        EncodedVerKey::from_did_and_verkey(&did, verkey)?
            .abbreviated_for_did(&did)
            .map_err(CliError::from)
    }

    pub fn qualify(store: &Wallet, did: &DidValue, method: &str) -> CliResult<DidValue> {
        block_on(async {
            let (entry, did_info) = Self::get_opt_record(store, &did.to_string(), true)
                .await?
                .ok_or_else(|| {
                    CliError::NotFound(format!("DID {} does not exits in the wallet!", did))
                })?;

            let qualified_did = did
                .to_qualified(method)
                .map_err(|_| CliError::InvalidInput(format!("Invalid DID {} provided.", did)))?;

            Self::remove(store, &did.to_string()).await?;

            let did_info = DidInfo {
                did: qualified_did.to_string(),
                ..did_info
            };

            let value = serde_json::to_vec(&did_info)?;
            store
                .store_record(CATEGORY_DID, &did_info.did, &value, Some(&entry.tags), true)
                .await?;

            Ok(qualified_did)
        })
    }

    pub async fn sign(store: &Wallet, did: &str, bytes: &[u8]) -> CliResult<Vec<u8>> {
        let (_, did_info) = Self::get_record(store, &did, true).await?;

        Key::sign(store, &did_info.verkey, bytes).await
    }

    async fn remove(store: &Wallet, name: &str) -> CliResult<()> {
        store.remove_record(CATEGORY_DID, name).await
    }

    pub async fn get_record(
        store: &Wallet,
        name: &str,
        for_update: bool,
    ) -> CliResult<(Entry, DidInfo)> {
        Self::get_opt_record(store, name, for_update)
            .await?
            .ok_or_else(|| {
                CliError::NotFound(format!("DID {} does not exits in the wallet.", name))
            })
    }

    async fn get_opt_record(
        store: &Wallet,
        name: &str,
        for_update: bool,
    ) -> CliResult<Option<(Entry, DidInfo)>> {
        let entry = store.fetch_record(CATEGORY_DID, name, for_update).await?;
        match entry {
            Some(entry) => {
                let did_info: DidInfo = serde_json::from_slice(&entry.value)?;
                Ok(Some((entry, did_info)))
            }
            None => Ok(None),
        }
    }
}
