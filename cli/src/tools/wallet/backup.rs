/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use std::{fs, path::PathBuf};

pub struct WalletBackup {}

impl WalletBackup {
    pub fn init_directory(path: &str) -> Result<(), std::io::Error> {
        if PathBuf::from(path).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Wallet backup \"{}\" already exists", path),
            ));
        }

        let path = PathBuf::from(path);
        fs::DirBuilder::new().recursive(true).create(path)
    }

    pub fn get_id(path: &str) -> String {
        let path = PathBuf::from(path);
        let id = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or("backup".to_string());
        id
    }

    pub fn is_wallet_backup_exist(path: &str) -> bool {
        PathBuf::from(path).exists()
    }
}
