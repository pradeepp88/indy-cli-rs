/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::error::{CliError, CliResult};

use hex::FromHex;
use indy_utils::base64;

const SEED_BYTES: usize = 32;

pub struct Seed(Vec<u8>);

impl Seed {
    pub fn value(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn from_str(seed: &str) -> CliResult<Seed> {
        if seed.as_bytes().len() == SEED_BYTES {
            // is acceptable seed length
            Ok(Seed(seed.as_bytes().to_vec()))
        } else if seed.ends_with('=') {
            // is base64 string
            let decoded = base64::decode(&seed)
                .map_err(|_| CliError::InvalidInput(format!("Invalid seed provided.")))?;
            if decoded.len() == SEED_BYTES {
                Ok(Seed(decoded))
            } else {
                Err(CliError::InvalidInput(format!(
                    "Provided invalid base64 encoded `seed`. \
                                   The number of bytes must be {} ",
                    SEED_BYTES
                )))
            }
        } else if seed.as_bytes().len() == SEED_BYTES * 2 {
            // is hex string
            let decoded = Vec::from_hex(seed)
                .map_err(|_| CliError::InvalidInput(format!("Seed is invalid hex")))?;
            Ok(Seed(decoded))
        } else {
            Err(CliError::InvalidInput(format!(
                "Provided invalid `seed`. It can be either \
                               {} bytes string or base64 string or {} bytes HEX string",
                SEED_BYTES,
                SEED_BYTES * 2
            )))
        }
    }
}
