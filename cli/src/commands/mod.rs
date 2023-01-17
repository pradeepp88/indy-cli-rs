extern crate serde_json;

pub mod common;
pub mod did;
pub mod ledger;
pub mod pool;
pub mod wallet;

use crate::command_executor::{CommandContext, CommandParams};

use crate::error::CliError;
use aries_askar::any::AnyStore;
use indy_utils::did::DidValue;
use indy_utils::Qualifiable;
use indy_vdr::pool::LocalPool;
use std;
use std::rc::Rc;

pub fn get_str_param<'a>(name: &'a str, params: &'a CommandParams) -> Result<&'a str, ()> {
    match params.get(name) {
        Some(v) if v == "" => {
            println_err!("Required \"{}\" parameter is empty.", name);
            Err(())
        }
        Some(v) => Ok(v.as_str()),
        None => {
            println_err!("No required \"{}\" parameter present.", name);
            Err(())
        }
    }
}

pub fn get_opt_str_param<'a>(
    key: &'a str,
    params: &'a CommandParams,
) -> Result<Option<&'a str>, ()> {
    match params.get(key) {
        Some(v) if v == "" => {
            println_err!("Optional parameter \"{}\" is empty.", key);
            Err(())
        }
        Some(v) => Ok(Some(v.as_str())),
        None => Ok(None),
    }
}

pub fn get_opt_empty_str_param<'a>(
    key: &'a str,
    params: &'a CommandParams,
) -> Result<Option<&'a str>, ()> {
    Ok(params.get(key).map(|v| v.as_str()))
}

pub fn _get_int_param<T>(name: &str, params: &CommandParams) -> Result<T, ()>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match params.get(name) {
        Some(v) => Ok(v.parse::<T>().map_err(|err| {
            println_err!("Can't parse integer parameter \"{}\": err {}", name, err)
        })?),
        None => {
            println_err!("No required \"{}\" parameter present", name);
            Err(())
        }
    }
}

pub fn get_number_param<T>(key: &str, params: &CommandParams) -> Result<T, ()>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match params.get(key) {
        Some(value) => value.parse::<T>().map_err(|err| {
            println_err!(
                "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                key,
                value,
                err
            )
        }),
        None => {
            println_err!("No required \"{}\" parameter present", key);
            Err(())
        }
    }
}

pub fn get_opt_number_param<T>(key: &str, params: &CommandParams) -> Result<Option<T>, ()>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let res = match params.get(key) {
        Some(value) => Some(value.parse::<T>().map_err(|err| {
            println_err!(
                "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                key,
                value,
                err
            )
        })?),
        None => None,
    };
    Ok(res)
}

pub fn get_bool_param(key: &str, params: &CommandParams) -> Result<bool, ()> {
    match params.get(key) {
        Some(value) => Ok(value
            .parse::<bool>()
            .map_err(|err| println_err!("Can't parse bool parameter \"{}\": err {}", key, err))?),
        None => {
            println_err!("No required \"{}\" parameter present", key);
            Err(())
        }
    }
}

pub fn get_opt_bool_param(key: &str, params: &CommandParams) -> Result<Option<bool>, ()> {
    match params.get(key) {
        Some(value) => Ok(Some(value.parse::<bool>().map_err(|err| {
            println_err!("Can't parse bool parameter \"{}\": err {}", key, err)
        })?)),
        None => Ok(None),
    }
}

pub fn get_str_array_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<Vec<&'a str>, ()> {
    match params.get(name) {
        None => {
            println_err!("No required \"{}\" parameter present", name);
            Err(())
        }
        Some(v) if v.is_empty() => {
            println_err!("No required \"{}\" parameter present", name);
            Err(())
        }
        Some(v) => Ok(v.split(',').collect::<Vec<&'a str>>()),
    }
}

pub fn get_opt_str_array_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<Option<Vec<&'a str>>, ()> {
    match params.get(name) {
        Some(v) => {
            if v.is_empty() {
                Ok(Some(Vec::<&'a str>::new()))
            } else {
                Ok(Some(v.split(',').collect::<Vec<&'a str>>()))
            }
        }
        None => Ok(None),
    }
}

pub fn get_object_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<serde_json::Value, ()> {
    match params.get(name) {
        Some(v) => Ok(serde_json::from_str(v).map_err(|err| {
            println_err!("Can't parse object parameter \"{}\": err {}", name, err)
        })?),
        None => {
            println_err!("No required \"{}\" parameter present", name);
            Err(())
        }
    }
}

pub fn get_opt_object_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<Option<serde_json::Value>, ()> {
    match params.get(name) {
        Some(_) => Ok(Some(get_object_param(name, params)?)),
        None => Ok(None),
    }
}

pub fn get_number_tuple_array_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<Vec<u64>, ()> {
    match params.get(name) {
        Some(v) if !v.is_empty() => {
            let tuples: Vec<&str> = v.split(",").collect();
            if tuples.is_empty() {
                println_err!("Parameter \"{}\" has invalid format", name);
                Err(())
            } else {
                let mut result: Vec<u64> = Vec::new();
                for item in tuples {
                    println!("{:?}", item);
                    result.push(item.parse::<u64>().map_err(|err| {
                        println_err!(
                            "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                            name,
                            item,
                            err
                        )
                    })?);
                }
                Ok(result)
            }
        }
        _ => {
            println_err!("No required \"{}\" parameter present", name);
            Err(())
        }
    }
}

pub fn convert_did(did: &str) -> Result<DidValue, ()> {
    DidValue::from_str(&did).map_err(|_| println_err!("Invalid DID {} provided", did))
}

pub fn ensure_active_did(ctx: &CommandContext) -> Result<DidValue, ()> {
    match ctx.get_string_value("ACTIVE_DID") {
        Some(did) => convert_did(&did),
        None => {
            println_err!("There is no active did");
            Err(())
        }
    }
}

pub fn get_active_did(ctx: &CommandContext) -> Result<Option<DidValue>, ()> {
    match ctx.get_string_value("ACTIVE_DID") {
        Some(did) => {
            let did = convert_did(&did)?;
            Ok(Some(did))
        }
        None => Ok(None),
    }
}

pub fn set_active_did(ctx: &CommandContext, did: String) {
    ctx.set_string_value("ACTIVE_DID", Some(did.clone()));
    ctx.set_sub_prompt(
        3,
        Some(format!("did({}...{})", &did[..3], &did[did.len() - 3..])),
    );
}

pub fn reset_active_did(ctx: &CommandContext) {
    ctx.set_string_value("ACTIVE_DID", None);
    ctx.set_sub_prompt(3, None);
}

pub fn get_did_param<'a>(name: &'a str, params: &'a CommandParams) -> Result<DidValue, ()> {
    let did_str = get_str_param(name, params)?;
    convert_did(did_str)
}

pub fn get_opt_did_param<'a>(
    name: &'a str,
    params: &'a CommandParams,
) -> Result<Option<DidValue>, ()> {
    let did_str = get_opt_str_param(name, params)?;
    match did_str {
        Some(did_str) => Ok(Some(convert_did(did_str)?)),
        None => Ok(None),
    }
}

pub fn ensure_opened_store(ctx: &CommandContext) -> Result<Rc<AnyStore>, ()> {
    match ctx.get_store_value() {
        Some(store) => Ok(store),
        None => {
            println_err!("There is no opened wallet now");
            Err(())
        }
    }
}

pub fn ensure_opened_wallet(ctx: &CommandContext) -> Result<(Rc<AnyStore>, String), ()> {
    let store = ctx.get_store_value();
    let name = ctx.get_string_value("OPENED_WALLET_NAME");

    match (store, name) {
        (Some(store), Some(name)) => Ok((store, name)),
        _ => {
            println_err!("There is no opened wallet now");
            Err(())
        }
    }
}

pub fn get_opened_wallet(ctx: &CommandContext) -> Option<(Rc<AnyStore>, String)> {
    let store = ctx.get_store_value();
    let name = ctx.get_string_value("OPENED_WALLET_NAME");

    if let (Some(store), Some(name)) = (store, name) {
        Some((store, name))
    } else {
        None
    }
}

pub fn set_opened_wallet(ctx: &CommandContext, value: Option<(AnyStore, String)>) {
    match value {
        Some((store, wallet_name)) => {
            ctx.set_store_value(Some(store));
            ctx.set_string_value("OPENED_WALLET_NAME", Some(wallet_name.to_owned()));
            ctx.set_sub_prompt(2, Some(wallet_name));
        }
        None => {
            ctx.set_store_value(None);
            ctx.set_string_value("OPENED_WALLET_NAME", None);
            ctx.set_sub_prompt(2, None);
        }
    }
}

pub fn ensure_connected_pool_handle(ctx: &CommandContext) -> Result<Rc<LocalPool>, ()> {
    match ctx.get_pool_value() {
        Some(pool) => Ok(pool),
        None => {
            println_err!("There is no opened pool now");
            Err(())
        }
    }
}

pub fn ensure_connected_pool(ctx: &CommandContext) -> Result<(Rc<LocalPool>, String), ()> {
    let handle = ctx.get_pool_value();
    let name = ctx.get_string_value("CONNECTED_POOL_NAME");

    match (handle, name) {
        (Some(handle), Some(name)) => Ok((handle, name)),
        _ => {
            println_err!("There is no opened pool now");
            Err(())
        }
    }
}

pub fn get_connected_pool(ctx: &CommandContext) -> Option<Rc<LocalPool>> {
    let pool = ctx.get_pool_value();

    if let Some(pool) = pool {
        Some(pool)
    } else {
        None
    }
}

pub fn get_connected_pool_with_name(ctx: &CommandContext) -> Option<(Rc<LocalPool>, String)> {
    let pool = ctx.get_pool_value();
    let name = ctx.get_string_value("CONNECTED_POOL_NAME");

    if let (Some(pool), Some(name)) = (pool, name) {
        Some((pool, name))
    } else {
        None
    }
}

pub fn set_connected_pool(ctx: &CommandContext, value: Option<(LocalPool, String)>) {
    ctx.set_string_value(
        "CONNECTED_POOL_NAME",
        value.as_ref().map(|value| value.1.to_owned()),
    );
    ctx.set_sub_prompt(1, value.as_ref().map(|value| format!("pool({})", value.1)));
    ctx.set_pool_value(value.map(|value| value.0));
}

pub fn set_transaction(ctx: &CommandContext, request: Option<String>) {
    ctx.set_string_value("LEDGER_TRANSACTION", request.clone());
}

pub fn get_transaction(ctx: &CommandContext) -> Option<String> {
    ctx.get_string_value("LEDGER_TRANSACTION")
}

pub fn ensure_set_transaction(ctx: &CommandContext) -> Result<String, ()> {
    match ctx.get_string_value("LEDGER_TRANSACTION") {
        Some(transaction) => Ok(transaction),
        None => {
            println_err!("There is no transaction stored into context");
            Err(())
        }
    }
}

pub fn set_transaction_author_info(ctx: &CommandContext, value: Option<(String, String, u64)>) {
    ctx.set_string_value(
        "AGREEMENT_TEXT",
        value.as_ref().map(|value| value.0.to_owned()),
    );
    ctx.set_string_value(
        "AGREEMENT_VERSION",
        value.as_ref().map(|value| value.1.to_owned()),
    );
    ctx.set_uint_value(
        "AGREEMENT_TIME_OF_ACCEPTANCE",
        value.as_ref().map(|value| value.2),
    );
}

pub fn get_transaction_author_info(ctx: &CommandContext) -> Option<(String, String, String, u64)> {
    let text = ctx.get_string_value("AGREEMENT_TEXT");
    let version = ctx.get_string_value("AGREEMENT_VERSION");
    let acc_mech_type = ctx.get_taa_acceptance_mechanism();
    let time_of_acceptance = ctx.get_uint_value("AGREEMENT_TIME_OF_ACCEPTANCE");

    if let (Some(text), Some(version), Some(time_of_acceptance)) =
        (text, version, time_of_acceptance)
    {
        Some((text, version, acc_mech_type, time_of_acceptance))
    } else {
        None
    }
}

const DEFAULT_POOL_PROTOCOL_VERSION: usize = 2;

pub fn set_pool_protocol_version(ctx: &CommandContext, protocol_version: usize) {
    ctx.set_uint_value("POOL_PROTOCOL_VERSION", Some(protocol_version as u64));
}

pub fn get_pool_protocol_version(ctx: &CommandContext) -> usize {
    match ctx.get_uint_value("POOL_PROTOCOL_VERSION") {
        Some(protocol_version) => protocol_version as usize,
        None => DEFAULT_POOL_PROTOCOL_VERSION,
    }
}

#[cfg(test)]
use crate::tools::ledger::Ledger;
#[cfg(test)]
use indy_vdr::pool::PreparedRequest;

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

    let pool = ensure_connected_pool_handle(ctx).unwrap();

    for _ in 0..SUBMIT_RETRY_CNT {
        let response = Ledger::submit_request(pool.as_ref(), request).unwrap();
        if parser(&response).is_ok() {
            return Ok(());
        }
        ::std::thread::sleep(::std::time::Duration::from_secs(SUBMIT_TIMEOUT_SEC));
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
