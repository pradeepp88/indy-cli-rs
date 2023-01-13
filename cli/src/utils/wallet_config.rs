use serde_json::Value as JsonValue;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::utils::environment::EnvironmentUtils;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub id: String,
    pub storage_type: String,
    pub storage_config: Option<JsonValue>,
}

pub struct WalletConfig {}

impl WalletConfig {
    fn init_directory(path: &PathBuf) -> Result<(), std::io::Error> {
        fs::DirBuilder::new().recursive(true).create(path)
    }

    pub(crate) fn store(id: &str, config: &Config) -> Result<(), std::io::Error> {
        Self::init_directory(&EnvironmentUtils::wallets_path())?;

        let path = EnvironmentUtils::wallet_config_path(id);

        let mut config_file = File::create(path)?;
        let config_json = json!(config).to_string();
        config_file.write_all(config_json.as_bytes())?;
        config_file.sync_all()?;

        Ok(())
    }

    pub(crate) fn read(id: &str) -> Result<Config, std::io::Error> {
        let path = EnvironmentUtils::wallet_config_path(id);

        let mut config_json = String::new();

        let mut file = File::open(path)?;
        file.read_to_string(&mut config_json)?;

        let config = serde_json::from_str(&config_json)?;
        Ok(config)
    }

    pub(crate) fn delete(id: &str) -> Result<(), std::io::Error> {
        let path = EnvironmentUtils::wallet_config_path(id);
        fs::remove_file(path)
    }
}
