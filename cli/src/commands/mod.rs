/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
pub mod common;
pub mod did;
pub mod ledger;
pub mod pool;
pub mod wallet;

use crate::command_executor::CommandContext;

use self::pool::constants::DEFAULT_POOL_PROTOCOL_VERSION;

use indy_utils::{did::DidValue, Qualifiable};
use std::rc::Rc;

impl CommandContext {
    pub fn ensure_active_did(&self) -> Result<DidValue, ()> {
        match self.get_string_value("ACTIVE_DID") {
            Some(did) => {
                DidValue::from_str(&did).map_err(|_| println_err!("Invalid DID {} provided", did))
            }
            None => {
                println_err!("There is no active did");
                Err(())
            }
        }
    }

    pub fn set_active_did(&self, did: String) {
        self.set_string_value("ACTIVE_DID", Some(did.clone()));
        self.set_sub_prompt(
            3,
            Some(format!("did({}...{})", &did[..3], &did[did.len() - 3..])),
        );
    }

    pub fn get_active_did(&self) -> Result<Option<DidValue>, ()> {
        match self.get_string_value("ACTIVE_DID") {
            Some(did) => {
                let did = DidValue::from_str(&did)
                    .map_err(|_| println_err!("Invalid DID {} provided", did))?;
                Ok(Some(did))
            }
            None => Ok(None),
        }
    }

    pub fn reset_active_did(&self) {
        self.set_string_value("ACTIVE_DID", None);
        self.set_sub_prompt(3, None);
    }

    pub fn set_opened_wallet(&self, (wallet, wallet_name): (Wallet, String)) {
        self.set_wallet_value(Some(wallet));
        self.set_string_value("OPENED_WALLET_NAME", Some(wallet_name.to_owned()));
        self.set_sub_prompt(2, Some(wallet_name));
    }

    pub fn ensure_opened_wallet(&self) -> Result<Rc<Wallet>, ()> {
        match self.get_wallet_value() {
            Some(wallet) => Ok(wallet),
            None => {
                println_err!("There is no opened wallet now");
                Err(())
            }
        }
    }

    pub fn ensure_opened_wallet_name(&self) -> Result<String, ()> {
        let name = self.get_string_value("OPENED_WALLET_NAME");

        match name {
            Some(name) => Ok(name),
            _ => {
                println_err!("There is no opened wallet now");
                Err(())
            }
        }
    }

    pub fn get_opened_wallet(&self) -> Option<(Rc<Wallet>, String)> {
        let wallet = self.get_wallet_value();
        let name = self.get_string_value("OPENED_WALLET_NAME");

        if let (Some(wallet), Some(name)) = (wallet, name) {
            Some((wallet, name))
        } else {
            None
        }
    }

    pub fn take_opened_wallet(&self) -> Option<(Wallet, String)> {
        let wallet = self.take_wallet_value();
        let name = self.get_string_value("OPENED_WALLET_NAME");

        if let (Some(wallet), Some(name)) = (wallet, name) {
            let wallet = Rc::try_unwrap(wallet).unwrap();
            Some((wallet, name))
        } else {
            None
        }
    }

    pub fn reset_wallet(&self) {
        self.set_wallet_value(None);
        self.set_string_value("OPENED_WALLET_NAME", None);
        self.set_sub_prompt(2, None);
    }

    pub fn set_connected_pool(&self, (pool, pool_name): (Pool, String)) {
        self.set_string_value("CONNECTED_POOL_NAME", Some(pool_name.clone()));
        self.set_sub_prompt(1, Some(format!("pool({})", pool_name)));
        self.set_pool_value(Some(pool));
    }

    pub fn ensure_connected_pool(&self) -> Result<Rc<Pool>, ()> {
        match self.get_pool_value() {
            Some(pool) => Ok(pool),
            None => {
                println_err!("There is no opened pool now");
                Err(())
            }
        }
    }

    pub fn ensure_connected_pool_name(&self) -> Result<String, ()> {
        let name = self.get_string_value("CONNECTED_POOL_NAME");

        match name {
            Some(name) => Ok(name),
            _ => {
                println_err!("There is no opened pool now");
                Err(())
            }
        }
    }

    pub fn get_connected_pool(&self) -> Option<Rc<Pool>> {
        let pool = self.get_pool_value();

        if let Some(pool) = pool {
            Some(pool)
        } else {
            None
        }
    }

    pub fn get_connected_pool_with_name(&self) -> Option<(Rc<Pool>, String)> {
        let pool = self.get_pool_value();
        let name = self.get_string_value("CONNECTED_POOL_NAME");

        if let (Some(pool), Some(name)) = (pool, name) {
            Some((pool, name))
        } else {
            None
        }
    }

    pub fn reset_connected_pool(&self) {
        self.set_string_value("CONNECTED_POOL_NAME", None);
        self.set_sub_prompt(1, None);
        self.set_pool_value(None);
    }

    pub fn set_context_transaction(&self, request: Option<String>) {
        self.set_string_value("LEDGER_TRANSACTION", request.clone());
    }

    pub fn get_context_transaction(&self) -> Option<String> {
        self.get_string_value("LEDGER_TRANSACTION")
    }

    pub fn ensure_context_transaction(&self) -> Result<String, ()> {
        match self.get_string_value("LEDGER_TRANSACTION") {
            Some(transaction) => Ok(transaction),
            None => {
                println_err!("There is no transaction stored into context");
                Err(())
            }
        }
    }

    pub fn set_transaction_author_info(&self, value: Option<(String, String, u64)>) {
        self.set_string_value(
            "AGREEMENT_TEXT",
            value.as_ref().map(|value| value.0.to_owned()),
        );
        self.set_string_value(
            "AGREEMENT_VERSION",
            value.as_ref().map(|value| value.1.to_owned()),
        );
        self.set_uint_value(
            "AGREEMENT_TIME_OF_ACCEPTANCE",
            value.as_ref().map(|value| value.2),
        );
    }

    pub fn get_transaction_author_info(&self) -> Option<(String, String, String, u64)> {
        let text = self.get_string_value("AGREEMENT_TEXT");
        let version = self.get_string_value("AGREEMENT_VERSION");
        let acc_mech_type = self.get_taa_acceptance_mechanism();
        let time_of_acceptance = self.get_uint_value("AGREEMENT_TIME_OF_ACCEPTANCE");

        if let (Some(text), Some(version), Some(time_of_acceptance)) =
            (text, version, time_of_acceptance)
        {
            Some((text, version, acc_mech_type, time_of_acceptance))
        } else {
            None
        }
    }

    pub fn set_pool_protocol_version(&self, protocol_version: usize) {
        self.set_uint_value("POOL_PROTOCOL_VERSION", Some(protocol_version as u64));
    }

    pub fn get_pool_protocol_version(&self) -> usize {
        match self.get_uint_value("POOL_PROTOCOL_VERSION") {
            Some(protocol_version) => protocol_version as usize,
            None => DEFAULT_POOL_PROTOCOL_VERSION,
        }
    }
}

#[cfg(test)]
use crate::tools::ledger::Ledger;
use crate::tools::{pool::Pool, wallet::Wallet};
#[cfg(test)]
use indy_vdr::pool::PreparedRequest;
#[cfg(test)]
use std::{thread::sleep, time};

#[cfg(test)]
pub fn submit_retry<F, T, E>(
    ctx: &CommandContext,
    request: &PreparedRequest,
    parser: F,
) -> Result<(), ()>
where
    F: Fn(&str) -> Result<T, E>,
{
    const SUBMIT_RETRY_CNT: usize = 3;
    const SUBMIT_TIMEOUT_SEC: u64 = 2;

    let pool = ctx.ensure_connected_pool().unwrap();

    for _ in 0..SUBMIT_RETRY_CNT {
        let response = Ledger::submit_request(pool.as_ref(), request).unwrap();
        if parser(&response).is_ok() {
            return Ok(());
        }
        sleep(time::Duration::from_secs(SUBMIT_TIMEOUT_SEC));
    }

    return Err(());
}

#[cfg(test)]
use crate::utils::test::TestUtils;

#[cfg(test)]
fn setup() -> CommandContext {
    TestUtils::cleanup_storage();
    CommandContext::new()
}

#[cfg(test)]
fn setup_with_wallet() -> CommandContext {
    let ctx = setup();
    wallet::tests::create_and_open_wallet(&ctx);
    ctx
}

#[cfg(test)]
fn setup_with_wallet_and_pool() -> CommandContext {
    let ctx = setup();
    wallet::tests::create_and_open_wallet(&ctx);
    pool::tests::create_and_connect_pool(&ctx);
    ctx
}

#[cfg(test)]
fn tear_down_with_wallet_and_pool(ctx: &CommandContext) {
    wallet::tests::close_and_delete_wallet(&ctx);
    pool::tests::disconnect_and_delete_pool(&ctx);
    tear_down();
}

#[cfg(test)]
fn tear_down_with_wallet(ctx: &CommandContext) {
    wallet::tests::close_and_delete_wallet(&ctx);
    tear_down();
}

#[cfg(test)]
fn tear_down() {
    TestUtils::cleanup_storage();
}
