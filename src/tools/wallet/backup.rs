/*
    Copyright © 2023 Province of British Columbia
    https://digital.gov.bc.ca/digital-trust
*/
use crate::error::{CliError, CliResult};
use std::{ffi::OsStr, fs, path::PathBuf};

pub struct WalletBackup {
    path: PathBuf,
}

#[derive(Debug)]
pub enum BackupKind {
    Askar,
    Libindy,
}

pub const DEFAULT_BACKUP_NAME: &'static str = "backup";

impl WalletBackup {
    pub fn from_file(path: &str) -> CliResult<Self> {
        let path = PathBuf::from(path);
        Ok(WalletBackup { path })
    }

    pub fn init_dir(&self) -> CliResult<()> {
        if self.exists() {
            return Err(CliError::Duplicate(format!(
                "Wallet backup \"{}\" already exists",
                self.path.to_string_lossy()
            )));
        }

        fs::DirBuilder::new()
            .recursive(true)
            .create(&self.path)
            .map_err(CliError::from)
    }

    pub fn id(&self) -> String {
        self.path
            .file_name()
            .and_then(OsStr::to_str)
            .map(String::from)
            .unwrap_or(DEFAULT_BACKUP_NAME.to_string())
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn kind(&self) -> CliResult<BackupKind> {
        let metadata = fs::metadata(&self.path)?;
        // if specified path to directory consider it as Askar backup
        if metadata.is_dir() {
            return Ok(BackupKind::Askar);
        }

        let extension = self.path.extension().and_then(OsStr::to_str);
        match extension {
            // if extension of backup file is `db` consider it as Askar backup
            Some("db") => Ok(BackupKind::Askar),
            // else consider it as a Libindy backup
            _ => Ok(BackupKind::Libindy),
        }
    }
}
