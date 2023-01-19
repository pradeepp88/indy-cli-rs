/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    ledger::handle_transaction_response,
    params_parser::ParamParser,
    tools::{
        did::Did,
        ledger::{Ledger, Response},
        pool::Pool,
        wallet::Wallet,
    },
};
use indy_utils::did::DidValue;

pub mod rotate_key_command {
    use super::*;
    use crate::{
        error::CliError,
        ledger::{handle_transaction_response, set_author_agreement},
        tools::ledger::Response,
    };
    use indy_vdr::common::error::VdrErrorKind;

    command!(
        CommandMetadata::build("rotate-key", "Rotate keys for active did")
            .add_optional_deferred_param(
                "seed",
                "If not provide then a random one will be created (UTF-8, base64 or hex)"
            )
            .add_optional_param("resume", "Resume interrupted operation")
            .add_example("did rotate-key")
            .add_example("did rotate-key seed=00000000000000000000000000000My2")
            .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let seed = ParamParser::get_opt_str_param("seed", params).map_err(error_err!())?;

        let resume = ParamParser::get_opt_bool_param("resume", params)
            .map_err(error_err!())?
            .unwrap_or(false);

        let did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool_with_name();
        let store = ctx.ensure_opened_wallet()?;

        // get verkey from ledger
        let ledger_verkey = match pool {
            Some((pool, pool_name)) => _get_current_verkey(&pool, &pool_name, &store, &did)?,
            None => None,
        };

        let is_did_on_the_ledger = ledger_verkey.is_some();

        let (new_verkey, update_ledger) = if resume {
            // get temp and current verkey from wallet.

            let did_info =
                Did::get(&store, &did).map_err(|err| println_err!("{}", err.message(None)))?;

            let temp_verkey = did_info.next_verkey.ok_or_else(|| {
                println_err!("Unable to resume, have you already run rotate-key?")
            })?;
            let curr_verkey = did_info.verkey;

            match ledger_verkey {
                Some(ledger_verkey) => {
                    // if ledger verkey is abbreviated, abbreviate other also.
                    let (temp_verkey, curr_verkey) = if ledger_verkey.starts_with('~') {
                        let temp_verkey = Did::abbreviate_verkey(&did, &temp_verkey)
                            .map_err(|_e| println_err!("Invalid temp verkey: {}", temp_verkey))?;
                        let curr_verkey =
                            Did::abbreviate_verkey(&did, &curr_verkey).map_err(|_e| {
                                println_err!("Invalid current verkey: {}", curr_verkey)
                            })?;
                        Ok((temp_verkey, curr_verkey))
                    } else {
                        Ok((temp_verkey, curr_verkey))
                    }?;

                    println_succ!("Verkey on ledger: {}", ledger_verkey);
                    println_succ!("Current verkey in wallet: {}", curr_verkey);
                    println_succ!("Temp verkey in wallet: {}", temp_verkey);

                    if ledger_verkey == temp_verkey {
                        // ledger is updated, need to apply change to wallet.
                        Ok((temp_verkey, false))
                    } else if ledger_verkey == curr_verkey {
                        // ledger have old state, send nym request and apply change to wallet.
                        Ok((temp_verkey, true))
                    } else {
                        // some invalid state
                        println_err!("Unable to resume, verkey on ledger is completely different from verkey in wallet");
                        Err(())
                    }
                }
                None => {
                    println_warn!("DID is not registered on the ledger");
                    Ok((temp_verkey, false))
                }
            }?
        } else {
            let new_verkey = Did::replace_keys_start(&store, &did, seed)
                .map_err(|err| println_err!("{}", err.message(None)))?;

            (new_verkey, true)
        };

        if update_ledger && is_did_on_the_ledger {
            let pool = ctx.ensure_connected_pool()?;
            let pool_name = ctx.ensure_connected_pool_name()?;
            let mut request =
                Ledger::build_nym_request(Some(&pool), &did, &did, Some(&new_verkey), None, None)
                    .map_err(|err| println_err!("{}", err.message(Some(&pool_name))))?;

            set_author_agreement(ctx, &mut request)?;

            let response_json = Ledger::sign_and_submit_request(&pool, &store, &did, &mut request)
                .map_err(|err| match err {
                    CliError::VdrError(ref vdr_err) => match vdr_err.kind() {
                        VdrErrorKind::PoolTimeout => {
                            println_err!("Transaction response has not been received");
                            println_err!("Use command `did rotate-key resume=true` to complete");
                        }
                        _ => {
                            println_err!("{}", err.message(Some(&pool_name)));
                        }
                    },
                    _ => {
                        println_err!("{}", err.message(Some(&pool_name)));
                    }
                })?;

            let response: Response<serde_json::Value> =
                serde_json::from_str::<Response<serde_json::Value>>(&response_json)
                    .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;

            handle_transaction_response(response)?;
        };

        Did::replace_keys_apply(&store, &did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let vk = Did::abbreviate_verkey(&did, &new_verkey)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        println_succ!("Verkey for did \"{}\" has been updated", did);
        println_succ!("New verkey is \"{}\"", vk);

        trace!("execute <<");
        Ok(())
    }
}

fn _get_current_verkey(
    pool: &Pool,
    pool_name: &str,
    store: &Wallet,
    did: &DidValue,
) -> Result<Option<String>, ()> {
    //TODO: There nym is requested. Due to freshness issues response might be stale or outdated. Something should be done with it
    let response_json = Ledger::build_get_nym_request(Some(pool), Some(did), did)
        .and_then(|mut request| Ledger::sign_and_submit_request(pool, store, did, &mut request))
        .map_err(|err| println_err!("{}", err.message(Some(pool_name))))?;
    let response: Response<serde_json::Value> =
        serde_json::from_str::<Response<serde_json::Value>>(&response_json)
            .map_err(|err| println_err!("Invalid data has been received: {:?}", err))?;
    let result = handle_transaction_response(response)?;
    let data = serde_json::from_str::<serde_json::Value>(&result["data"].as_str().unwrap_or("{}"))
        .map_err(|_| println_err!("Wrong data has been received"))?;
    let verkey = data["verkey"].as_str().map(String::from);
    Ok(verkey)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{commands::ledger::tests::send_nym, tools::did::Did};

    mod did_rotate_key {
        use super::*;
        use crate::{
            commands::{
                setup, setup_with_wallet_and_pool, submit_retry, tear_down,
                tear_down_with_wallet_and_pool,
            },
            did::tests::{get_did_info, new_did, use_did, DID_TRUSTEE, SEED_TRUSTEE},
            pool::tests::{create_and_connect_pool, disconnect_and_delete_pool},
            wallet::tests::{close_and_delete_wallet, create_and_open_wallet},
        };

        fn ensure_nym_written(ctx: &CommandContext, did: &str, verkey: &str) {
            let pool = ctx.get_connected_pool().unwrap();
            let wallet = ctx.ensure_opened_wallet().unwrap();
            let did = DidValue(did.to_string());
            let mut request = Ledger::build_get_nym_request(Some(&pool), None, &did).unwrap();
            Ledger::sign_request(&wallet, &did, &mut request).unwrap();
            submit_retry(ctx, &request, |response| {
                let res = req_for_nym(response);
                match res {
                    Some(ref verkey_received) if verkey_received == verkey => Ok(()),
                    _ => Err(()),
                }
            })
            .unwrap()
        }

        fn req_for_nym(response: &str) -> Option<String> {
            let parsed = serde_json::from_str::<serde_json::Value>(&response).ok()?;
            let data = parsed["result"]["data"].as_str()?;
            let data = serde_json::from_str::<serde_json::Value>(&data).ok()?;
            let verkey = data["verkey"].as_str()?;
            Some(verkey.to_string())
        }

        #[test]
        pub fn rotate_works() {
            let ctx = setup_with_wallet_and_pool();

            new_did(&ctx, SEED_TRUSTEE);

            let wallet = ctx.ensure_opened_wallet().unwrap();
            let (did, verkey) = Did::create(&wallet, None, None, None, None).unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            ensure_nym_written(&ctx, &did, &verkey);
            use_did(&ctx, &did);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey);
            {
                let cmd = rotate_key_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_ne!(did_info.verkey, verkey);

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn rotate_resume_works_when_ledger_updated() {
            let ctx = setup();

            let wallet = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);
            let pool = ctx.ensure_connected_pool().unwrap();

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::create(&wallet, None, None, None, None).unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            let new_verkey = Did::replace_keys_start(&wallet, &did, None).unwrap();
            let did = DidValue(did.to_string());
            let mut request =
                Ledger::build_nym_request(Some(&pool), &did, &did, Some(&new_verkey), None, None)
                    .unwrap();
            Ledger::sign_and_submit_request(&pool, &wallet, &did, &mut request).unwrap();
            ensure_nym_written(&ctx, &did, &new_verkey);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey);
            assert_eq!(did_info.next_verkey.unwrap(), new_verkey);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, new_verkey);
            assert_eq!(did_info.next_verkey, None);

            close_and_delete_wallet(&ctx);
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn rotate_resume_works_when_ledger_not_updated() {
            let ctx = setup();

            let wallet = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::create(&wallet, None, None, None, None).unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);
            ensure_nym_written(&ctx, &did, &verkey);

            let new_verkey = Did::replace_keys_start(&wallet, &did, None).unwrap();

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey);
            assert_eq!(did_info.next_verkey.unwrap(), new_verkey);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, new_verkey);
            assert_eq!(did_info.next_verkey, None);

            close_and_delete_wallet(&ctx);
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn rotate_resume_without_started_rotation_rejected() {
            let ctx = setup();

            let wallet = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::create(&wallet, None, None, None, None).unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey);
            assert_eq!(did_info.next_verkey, None);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey); // it is not changed.
            assert_eq!(did_info.next_verkey, None);

            close_and_delete_wallet(&ctx);
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn rotate_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = rotate_key_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
