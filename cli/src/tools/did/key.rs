/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    error::{CliError, CliResult},
    tools::did::seed::Seed,
};

use crate::tools::wallet::Wallet;
use aries_askar::kms::{KeyAlg, LocalKey};
use indy_utils::base58;

pub struct Key(LocalKey);

impl Key {
    pub async fn create(
        store: &Wallet,
        seed: Option<&str>,
        metadata: Option<&str>,
    ) -> CliResult<Key> {
        let keypair = match seed {
            Some(seed) => {
                let seed = Seed::from_str(seed)?;
                LocalKey::from_secret_bytes(KeyAlg::Ed25519, seed.value())?
            }
            None => LocalKey::generate(KeyAlg::Ed25519, false)?,
        };

        let key = Key(keypair);

        let verkey = key.verkey()?;

        store.insert_key(&verkey, &key, metadata).await?;

        Ok(key)
    }

    pub fn value(&self) -> &LocalKey {
        &self.0
    }

    pub fn verkey(&self) -> CliResult<String> {
        let public_key = self.0.to_public_bytes()?;
        Ok(base58::encode(public_key))
    }

    pub async fn sign(store: &Wallet, id: &str, bytes: &[u8]) -> CliResult<Vec<u8>> {
        store
            .fetch_key(id)
            .await?
            .sign_message(bytes, None)
            .map_err(CliError::from)
    }
}
