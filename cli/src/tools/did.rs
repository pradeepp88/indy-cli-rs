use crate::error::{CliError, CliResult};

use aries_askar::any::AnyStore;
use aries_askar::{
    future::block_on,
    kms::{KeyAlg, LocalKey},
    Entry, EntryTag,
};
use indy_utils::{base58, did::DidValue, keys::EncodedVerKey, Qualifiable};

pub struct Did {}

const CATEGORY_DID: &'static str = "did";
const KEY_TYPE: &'static str = "ed25519";

#[derive(Serialize, Deserialize)]
pub struct DidInfo {
    pub did: String,
    pub verkey: String,
    pub verkey_type: String,
    pub method: Option<String>,
    pub metadata: Option<String>,
    pub next_verkey: Option<String>,
}

impl Did {
    pub fn new(
        store: &AnyStore,
        did: Option<&str>,
        seed: Option<&str>,
        metadata: Option<&str>,
        method: Option<&str>,
    ) -> CliResult<(String, String)> {
        block_on(async move {
            let (keypair, verkey) = Did::create_key(store, seed, metadata).await?;

            let public_key = keypair.to_public_bytes()?;
            let did = match did {
                Some(did) => did.to_string(),
                None => base58::encode(&public_key[0..16])
            };

            let existing_did = Self::fetch_did(store, &did, false).await?;
            if existing_did.is_some() {
                return Err(CliError::Duplicate(format!(
                    "DID already present in the wallet!"
                )));
            }

            let mut tags = vec![
                EntryTag::Encrypted("verkey".to_string(), verkey.clone()),
                EntryTag::Encrypted("verkey_type".to_string(), KEY_TYPE.to_string()),
            ];
            if let Some(method) = method {
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

            Self::store_did(store, &did_info, Some(&tags), true).await?;

            Ok((did, verkey))
        })
    }

    pub fn replace_keys_start(
        store: &AnyStore,
        did: &str,
        seed: Option<&str>,
    ) -> CliResult<String> {
        block_on(async move {
            let (did_entry, mut did_info) =
                Self::fetch_did(store, &did, true).await?.ok_or_else(|| {
                    CliError::NotFound(format!("DID {} does not exits in the wallet.", did))
                })?;

            let (_, verkey) = Did::create_key(store, seed, None).await?;

            did_info.next_verkey = Some(verkey.clone());

            Self::store_did(store, &did_info, Some(&did_entry.tags), false).await?;

            Ok(verkey)
        })
    }

    pub fn replace_keys_apply(store: &AnyStore, did: &str) -> CliResult<()> {
        block_on(async move {
            let (did_entry, mut did_info) =
                Self::fetch_did(store, &did, true).await?.ok_or_else(|| {
                    CliError::NotFound(format!("DID {} does not exits in the wallet.", did))
                })?;

            let next_verkey = did_info.next_verkey.ok_or_else(|| {
                CliError::InvalidEntityState(format!("Next key is not set for the DID {}.", did))
            })?;

            did_info.verkey = next_verkey;
            did_info.next_verkey = None;

            Self::store_did(store, &did_info, Some(&did_entry.tags), false).await?;

            Ok(())
        })
    }

    pub fn set_metadata(store: &AnyStore, did: &str, metadata: &str) -> CliResult<()> {
        block_on(async move {
            let (did_entry, mut did_info) =
                Self::fetch_did(store, &did, true).await?.ok_or_else(|| {
                    CliError::NotFound(format!("DID {} does not exits in the wallet.", did))
                })?;

            did_info.metadata = Some(metadata.to_string());

            Self::store_did(store, &did_info, Some(&did_entry.tags), false).await?;

            Ok(())
        })
    }

    pub fn get_did_with_meta(store: &AnyStore, did: &DidValue) -> CliResult<DidInfo> {
        block_on(async move {
            let (_, did_info) = Self::fetch_did(store, &did.to_string(), true)
                .await?
                .ok_or_else(|| {
                    CliError::NotFound(format!("DID {} does not exits in the wallet.", did))
                })?;

            Ok(did_info)
        })
    }

    pub fn list_dids_with_meta(store: &AnyStore) -> CliResult<Vec<DidInfo>> {
        block_on(async move {
            let mut session = store.session(None).await?;

            session
                .fetch_all(CATEGORY_DID, None, None, false)
                .await?
                .iter()
                .map(|did| serde_json::from_slice(&did.value).map_err(CliError::from))
                .collect::<CliResult<Vec<DidInfo>>>()
        })
    }

    pub fn abbreviate_verkey(did: &str, verkey: &str) -> CliResult<String> {
        EncodedVerKey::from_did_and_verkey(did, verkey)?
            .abbreviated_for_did(did)
            .map_err(CliError::from)
    }

    pub fn qualify_did(did: &str, method: &str) -> CliResult<String> {
        DidValue(did.to_string())
            .to_qualified(method)
            .map(|did| did.to_string())
            .map_err(|_| CliError::InvalidInput(format!("Invalid DID {} provided.", did)))
    }

    pub async fn sign(store: &AnyStore, did: &str, bytes: &[u8]) -> CliResult<Vec<u8>> {
        Did::load_key(store, did)
            .await?
            .sign_message(bytes, None)
            .map_err(CliError::from)
    }

    async fn create_key(
        store: &AnyStore,
        seed: Option<&str>,
        metadata: Option<&str>,
    ) -> CliResult<(LocalKey, String)> {
        let keypair = match seed {
            Some(seed) => {
                println!("{:?}", seed);
                LocalKey::from_secret_bytes(KeyAlg::Ed25519, seed.as_bytes())?
            }
            None => LocalKey::generate(KeyAlg::Ed25519, false)?,
        };

        let public_key = keypair.to_public_bytes()?;
        let secret_key = keypair.to_secret_bytes()?;
        let verkey = base58::encode(public_key);
        let secret = base58::encode(secret_key);
        println!("{}", verkey);
        println!("{}", secret);

        let mut session = store.session(None).await?;
        session
            .insert_key(&verkey, &keypair, metadata, None, None)
            .await?;

        Ok((keypair, verkey))
    }

    async fn store_did(
        store: &AnyStore,
        did: &DidInfo,
        tags: Option<&[EntryTag]>,
        new: bool,
    ) -> CliResult<()> {
        let mut session = store.session(None).await?;

        let value_bytes = serde_json::to_vec(&did)?;

        if new {
            session
                .insert(CATEGORY_DID, &did.did, &value_bytes, tags, None)
                .await
                .map_err(CliError::from)
        } else {
            session
                .replace(CATEGORY_DID, &did.did, &value_bytes, tags, None)
                .await
                .map_err(CliError::from)
        }
    }

    async fn fetch_did(
        store: &AnyStore,
        name: &str,
        for_update: bool,
    ) -> CliResult<Option<(Entry, DidInfo)>> {
        let mut session = store.session(None).await?;
        let entry = session.fetch(CATEGORY_DID, &name, for_update).await?;
        match entry {
            Some(entry) => {
                let did_info: DidInfo = serde_json::from_slice(&entry.value)?;
                Ok(Some((entry, did_info)))
            }
            None => Ok(None),
        }
    }

    async fn load_key(store: &AnyStore, did: &str) -> CliResult<LocalKey> {
        let mut session = store.session(None).await?;

        let (_, did_info) = Self::fetch_did(store, &did, true).await?.ok_or_else(|| {
            CliError::NotFound(format!("DID {} does not exits in the wallet!", did))
        })?;

        session
            .fetch_key(&did_info.verkey, false)
            .await?
            .ok_or_else(|| {
                CliError::NotFound(format!("Key for DID {} does not exits in the wallet!", did))
            })?
            .load_local_key()
            .map_err(CliError::from)
    }
}
