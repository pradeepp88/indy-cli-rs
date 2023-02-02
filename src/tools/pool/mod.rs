/*
    Copyright © 2023 Province of British Columbia
    https://digital.gov.bc.ca/digital-trust
*/
use crate::{
    error::{CliError, CliResult},
    utils::futures::block_on,
};
use std::collections::HashMap;

use indy_vdr::{
    config::PoolConfig as OpenPoolConfig,
    pool::{helpers::perform_refresh, LocalPool, Pool as PoolImpl, PoolBuilder, PoolTransactions},
};
use pool_config::{PoolConfig, PoolDirectory};

pub mod pool_config;

pub struct Pool {
    pub pool: LocalPool,
    pub name: String,
}

impl Pool {
    pub fn create(name: &str, config: &PoolConfig) -> CliResult<()> {
        PoolDirectory::from(name)
            .store_config(config)
            .map_err(CliError::from)
    }

    pub fn open(
        name: &str,
        config: OpenPoolConfig,
        pre_ordered_nodes: Option<Vec<&str>>,
    ) -> CliResult<Pool> {
        let pool_transactions_file = PoolDirectory::from(name)
            .read_config()
            .map_err(|_| CliError::NotFound(format!("Pool \"{}\" does not exist.", name)))?
            .genesis_txn;

        let weight_nodes = pre_ordered_nodes.map(|pre_ordered_nodes| {
            pre_ordered_nodes
                .into_iter()
                .map(|node| (node.to_string(), 2.0))
                .collect::<HashMap<String, f32>>()
        });

        let pool_transactions = PoolTransactions::from_json_file(&pool_transactions_file)?;

        let pool = PoolBuilder::from(config)
            .transactions(pool_transactions)?
            .node_weights(weight_nodes)
            .into_local()?;

        let pool = Pool {
            pool,
            name: name.to_string(),
        };

        pool.refresh()
            .map_err(|_| CliError::NotFound(format!("Unable to connect to pool \"{}\"", name)))?;

        Ok(pool)
    }

    pub fn refresh(&self) -> CliResult<Option<Pool>> {
        let (transactions, _) = block_on(async move { perform_refresh(&self.pool).await })?;

        match transactions {
            Some(new_transactions) if new_transactions.len() > 0 => {
                let mut transactions = PoolTransactions::from(self.pool.get_merkle_tree());
                transactions.extend_from_json(new_transactions)?;

                let pool = PoolBuilder::from(self.pool.get_config().to_owned())
                    .transactions(transactions)?
                    .into_local()?;

                PoolDirectory::from(&self.name)
                    .store_pool_transactions(&self.pool.get_json_transactions()?)?;

                Ok(Some(Pool {
                    pool,
                    name: self.name.to_string(),
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn list() -> CliResult<String> {
        PoolDirectory::list_pools().map_err(CliError::from)
    }

    pub fn close(&self) -> CliResult<()> {
        Ok(())
    }

    pub fn delete(name: &str) -> CliResult<()> {
        PoolDirectory::from(name)
            .delete_config()
            .map_err(CliError::from)
    }
}
