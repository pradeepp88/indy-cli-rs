use crate::utils::environment::EnvironmentUtils;
use std::fs::File;
use std::io::{Read, Write};
use std::{fs, io};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub genesis_txn: String,
}

pub struct PoolConfig {}

impl PoolConfig {
    pub(crate) fn store(name: &str, config: &Config) -> Result<(), std::io::Error> {
        let mut path = EnvironmentUtils::pool_path(name);

        if path.as_path().exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Pool {} already exists!", name),
            ));
        }

        fs::create_dir_all(path.as_path())?;

        // copy genesis transactions
        {
            path.push(name);
            path.set_extension("txn");

            let mut gt_fin = File::open(&config.genesis_txn)?;
            let mut gt_fout = File::create(path.as_path())?;
            io::copy(&mut gt_fin, &mut gt_fout)?;
        }
        let txn_path = path.to_string_lossy().to_string();

        path.pop();

        // store config file
        {
            path.push(name);
            path.set_extension("json");

            let pool_config = json!({
                "genesis_txn": txn_path
            });

            let mut f: File = File::create(path.as_path())?;
            f.write_all(pool_config.to_string().as_bytes())?;
            f.flush()?;
        }

        Ok(())
    }

    pub(crate) fn write_transactions(name: &str, transactions: &Vec<String>) -> Result<(), std::io::Error> {
        let path = EnvironmentUtils::pool_transactions_path(name);
        let mut f = File::create(path.as_path())?;
        f.write_all(transactions.join("\n").as_bytes())?;
        Ok(())
    }

    pub(crate) fn read(id: &str) -> Result<Config, std::io::Error> {
        let path = EnvironmentUtils::pool_config_path(id);

        let mut config_json = String::new();

        let mut file = File::open(path)?;
        file.read_to_string(&mut config_json)?;

        let config = serde_json::from_str(&config_json)?;
        Ok(config)
    }

    pub(crate) fn delete(name: &str) -> Result<(), std::io::Error> {
        let path = EnvironmentUtils::pool_path(name);
        if !path.as_path().exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Pool \"{}\" does not exist.", name),
            ));
        }
        fs::remove_dir_all(path)
    }

    pub(crate) fn list() -> Result<String, std::io::Error> {
        let mut pools = Vec::new();
        let pool_home_path = EnvironmentUtils::pool_home_path();

        if let Ok(entries) = fs::read_dir(pool_home_path) {
            for entry in entries {
                let dir_entry = if let Ok(dir_entry) = entry {
                    dir_entry
                } else {
                    continue;
                };
                if let Some(pool_name) = dir_entry
                    .path()
                    .file_name()
                    .and_then(|os_str| os_str.to_str())
                {
                    let json = json!({ "pool": pool_name.to_owned() });
                    pools.push(json);
                }
            }
        }

        let pools = json!(pools).to_string();
        Ok(pools)
    }
}
