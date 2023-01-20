/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::did::Did,
};

pub mod rotate_key_command {
    use super::*;
    use crate::{
        error::CliError,
        ledger::{get_current_verkey, send_nym},
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

        let seed = ParamParser::get_opt_str_param("seed", params)?;

        let resume = ParamParser::get_opt_bool_param("resume", params)?.unwrap_or(false);

        let did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();
        let store = ctx.ensure_opened_wallet()?;

        // get verkey from ledger
        let ledger_verkey = match pool {
            Some(pool) => get_current_verkey(&pool, &store, &did)?,
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

            send_nym(&ctx, &pool, &store, &did, &new_verkey).map_err(|err| match err {
                CliError::VdrError(ref vdr_err) => match vdr_err.kind() {
                    VdrErrorKind::PoolTimeout => {
                        println_err!("Transaction response has not been received");
                        println_err!("Use command `did rotate-key resume=true` to complete");
                    }
                    _ => {
                        println_err!("{}", err.message(Some(&pool.name)));
                    }
                },
                _ => {
                    println_err!("{}", err.message(Some(&pool.name)));
                }
            })?;
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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::tools::did::Did;

    mod did_rotate_key {
        use super::*;
        use crate::{
            commands::{setup_with_wallet_and_pool, submit_retry, tear_down_with_wallet_and_pool},
            did::tests::get_did_info,
            ledger::tests::use_new_identity,
            tools::ledger::Ledger,
        };
        use indy_utils::did::DidValue;

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

            let (did, verkey) = use_new_identity(&ctx);
            ensure_nym_written(&ctx, &did, &verkey);

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
            let ctx = setup_with_wallet_and_pool();

            let (did, verkey) = use_new_identity(&ctx);

            // start key rotation and update ledger
            let new_verkey = {
                let pool = ctx.ensure_connected_pool().unwrap();
                let wallet = ctx.ensure_opened_wallet().unwrap();
                let new_verkey = Did::replace_keys_start(&wallet, &did, None).unwrap();
                let did = DidValue(did.to_string());
                let mut request = Ledger::build_nym_request(
                    Some(&pool),
                    &did,
                    &did,
                    Some(&new_verkey),
                    None,
                    None,
                )
                .unwrap();
                Ledger::sign_and_submit_request(&pool, &wallet, &did, &mut request).unwrap();
                ensure_nym_written(&ctx, &did, &new_verkey);
                new_verkey
            };

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

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn rotate_resume_works_when_ledger_not_updated() {
            let ctx = setup_with_wallet_and_pool();

            let (did, verkey) = use_new_identity(&ctx);

            let new_verkey = {
                let wallet = ctx.ensure_opened_wallet().unwrap();
                Did::replace_keys_start(&wallet, &did, None).unwrap()
            };

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

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn rotate_resume_without_started_rotation_rejected() {
            let ctx = setup_with_wallet_and_pool();

            let (did, verkey) = use_new_identity(&ctx);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info.verkey, verkey);
            assert_eq!(did_info.next_verkey, None);

            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            {
                let did_info = get_did_info(&ctx, &did);
                assert_eq!(did_info.verkey, verkey); // it is not changed.
                assert_eq!(did_info.next_verkey, None);
            }

            tear_down_with_wallet_and_pool(&ctx);
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
