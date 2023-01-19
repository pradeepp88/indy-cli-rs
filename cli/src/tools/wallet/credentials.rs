/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    error::{CliError, CliResult},
    tools::wallet::Credentials,
};

use aries_askar::{Argon2Level, KdfMethod, PassKey, StoreKeyMethod};

pub struct WalletCredentials<'a> {
    pub key: PassKey<'a>,
    pub key_method: StoreKeyMethod,
    pub rekey: Option<PassKey<'a>>,
    pub rekey_method: Option<StoreKeyMethod>,
}

impl<'a> WalletCredentials<'a> {
    pub fn build(credentials: &Credentials) -> CliResult<WalletCredentials> {
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

        Ok(WalletCredentials {
            key,
            key_method,
            rekey,
            rekey_method,
        })
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
