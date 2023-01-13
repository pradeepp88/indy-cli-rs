use crate::command_executor::{
    Command, CommandContext, CommandGroup, CommandGroupMetadata, CommandMetadata, CommandParams,
    DynamicCompletionType,
};
use crate::commands::*;
use crate::utils::table::print_list_table;

use crate::tools::did::Did;
use crate::tools::ledger::Ledger;

use serde_json::Value as JSONValue;
use std::fs::File;

use crate::commands::ledger::{handle_transaction_response, Response};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "did",
        "Identity management commands"
    ));
}

pub mod new_command {
    use super::*;

    command!(CommandMetadata::build("new", "Create new DID")
        .add_optional_param("did", "Known DID for new wallet instance")
        .add_optional_deferred_param(
            "seed",
            "Seed for creating DID key-pair (UTF-8, base64 or hex)"
        )
        .add_optional_param("method", "Method name to create fully qualified DID")
        .add_optional_param("metadata", "DID metadata")
        .add_example("did new")
        .add_example("did new did=VsKV7grR1BUE29mG2Fm2kX")
        .add_example("did new did=VsKV7grR1BUE29mG2Fm2kX method=indy")
        .add_example("did new did=VsKV7grR1BUE29mG2Fm2kX seed=00000000000000000000000000000My1")
        .add_example("did new seed=00000000000000000000000000000My1 metadata=did_metadata")
        .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, secret!(params));

        let store = ensure_opened_store(&ctx)?;

        let did = get_opt_str_param("did", params).map_err(error_err!())?;
        let seed = get_opt_str_param("seed", params).map_err(error_err!())?;
        let method = get_opt_str_param("method", params).map_err(error_err!())?;
        let metadata = get_opt_empty_str_param("metadata", params).map_err(error_err!())?;

        let (did, vk) = Did::new(&store, did, seed, metadata, method)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let vk = Did::abbreviate_verkey(&did, &vk)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        println_succ!("Did \"{}\" has been created with \"{}\" verkey", did, vk);

        if let Some(metadata) = metadata {
            Did::set_metadata(&store, &did, metadata)
                .map_err(|err| println_err!("{}", err.message(None)))?;
        }

        trace!("execute <<");
        Ok(())
    }
}

pub mod import_command {
    use super::*;
    use std::io::Read;

    command!(CommandMetadata::build(
        "import",
        "Import DIDs entities from file to the current wallet.
        File format:
        {
            \"version\": 1,
            \"dids\": [{
                \"did\": \"did\",
                \"seed\": \"UTF-8, base64 or hex string\"
            }]
        }"
    )
    .add_main_param("file", "Path to file with DIDs")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let store = ensure_opened_store(&ctx)?;

        let path = get_str_param("file", params).map_err(error_err!())?;

        let mut buf = String::new();

        File::open(path)
            .and_then(|mut file| file.read_to_string(&mut buf))
            .map_err(|err| println_err!("Error during reading file {}", err))?;

        let json = serde_json::from_str::<JSONValue>(&buf)
            .map_err(|err| println_err!("Can't parse JSON {:?}", err))?;

        let is_correct_version = json["version"]
            .as_i64()
            .map(|ver| (ver == 1))
            .unwrap_or(false);

        if !is_correct_version {
            println_err!("Invalid or missed version");
            return Err(());
        }

        let dids = json["dids"]
            .as_array()
            .map(Clone::clone)
            .ok_or_else(|| println_err!("missed DIDs"))?;

        for did in dids {
            let (did, vk) = Did::new(&store, did["did"].as_str(), did["seed"].as_str(), None, None)
                .map_err(|err| println_err!("{}", err.message(None)))?;

            let vk = Did::abbreviate_verkey(&did, &vk)
                .map_err(|err| println_err!("{}", err.message(None)))?;

            println_succ!("Did \"{}\" has been created with \"{}\" verkey", did, vk)
        }

        println_succ!("DIDs import finished");

        trace!("execute << ");
        Ok(())
    }
}

pub mod use_command {
    use super::*;

    command!(CommandMetadata::build("use", "Use DID")
        .add_main_param_with_dynamic_completion(
            "did",
            "Did stored in wallet",
            DynamicCompletionType::Did
        )
        .add_example("did use VsKV7grR1BUE29mG2Fm2kX")
        .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?}, params {:?}", ctx, params);

        let did = get_did_param("did", params).map_err(error_err!())?;

        let store = ensure_opened_store(ctx)?;

        Did::get_did_with_meta(&store, &did)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        set_active_did(ctx, did.to_string());
        println_succ!("Did \"{}\" has been set as active", did);

        trace!("execute <<");
        Ok(())
    }
}

pub mod rotate_key_command {
    use super::*;
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

        let seed = get_opt_str_param("seed", params).map_err(error_err!())?;

        let resume = get_opt_bool_param("resume", params)
            .map_err(error_err!())?
            .unwrap_or(false);

        let did = ensure_active_did(&ctx)?;
        let (pool, pool_name) = ensure_connected_pool(&ctx)?;
        let (store, _) = ensure_opened_wallet(&ctx)?;

        // get verkey from ledger
        let ledger_verkey = _get_current_verkey(&pool, &pool_name, &store, &did)?;
        let is_did_on_the_ledger = ledger_verkey.is_some();

        let (new_verkey, update_ledger) = if resume {
            // get temp and current verkey from wallet.

            let did_info = Did::get_did_with_meta(&store, &did)
                .map_err(|err| println_err!("{}", err.message(None)))?;

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
            let mut request =
                Ledger::build_nym_request(Some(&pool), &did, &did, Some(&new_verkey), None, None)
                    .map_err(|err| println_err!("{}", err.message(Some(&pool_name))))?;

            ledger::set_author_agreement(ctx, &mut request)?;

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
    pool: &LocalPool,
    pool_name: &str,
    store: &AnyStore,
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

pub mod list_command {
    use super::*;

    command!(
        CommandMetadata::build("list", "List my DIDs stored in the opened wallet.").finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let store = ensure_opened_store(&ctx)?;

        let mut dids = Did::list_dids_with_meta(&store)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        for did_info in dids.iter_mut() {
            did_info.verkey = Did::abbreviate_verkey(&did_info.did, &did_info.verkey)
                .map_err(|err| println_err!("{}", err.message(None)))?;
        }

        print_list_table(
            &dids
                .iter()
                .map(|did| json!(did))
                .collect::<Vec<serde_json::Value>>(),
            &[
                ("did", "Did"),
                ("verkey", "Verkey"),
                ("metadata", "Metadata"),
            ],
            "There are no dids",
        );
        if let Some(cur_did) = get_active_did(ctx)? {
            println_succ!("Current did \"{}\"", cur_did);
        }

        trace!("execute <<");
        Ok(())
    }
}

pub mod qualify_command {
    use super::*;

    command!(CommandMetadata::build(
        "qualify",
        "Update DID stored in the wallet to make fully qualified, or to do other DID maintenance."
    )
    .add_main_param_with_dynamic_completion(
        "did",
        "Did stored in wallet",
        DynamicCompletionType::Did
    )
    .add_required_param("method", "Method to apply to the DID.")
    .add_example("did qualify VsKV7grR1BUE29mG2Fm2kX method=did:peer")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?}, params {:?}", ctx, params);

        let did = get_did_param("did", params).map_err(error_err!())?;
        let method = get_str_param("method", params).map_err(error_err!())?;

        let full_qualified_did = Did::qualify_did(&did, &method)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        println_succ!("Fully qualified DID \"{}\"", full_qualified_did);

        if let Some(active_did) = get_active_did(&ctx)? {
            if active_did == did {
                set_active_did(ctx, full_qualified_did.to_owned());
                println_succ!("Target DID is the same as CLI active. Active DID has been updated");
            }
        }

        trace!("execute <<");
        Ok(())
    }
}

pub fn did_list(ctx: &CommandContext) -> Vec<String> {
    get_opened_wallet(ctx)
        .and_then(|(store, _)| Did::list_dids_with_meta(&store).ok())
        .unwrap_or(vec![])
        .into_iter()
        .map(|did| did.did)
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::ledger::tests::send_nym;
    use crate::commands::pool::tests::{create_and_connect_pool, disconnect_and_delete_pool};
    use crate::commands::wallet::tests::{close_and_delete_wallet, create_and_open_wallet};
    use crate::tools::did::Did;

    pub const SEED_TRUSTEE: &'static str = "000000000000000000000000Trustee1";
    pub const DID_TRUSTEE: &'static str = "V4SGRU86Z58d6TV7PBUe6f";
    pub const VERKEY_TRUSTEE: &'static str = "GJ1SzoWzavQYfNL9XkaJdrQejfztN4XqdsiV4ct3LXKL";

    pub const SEED_MY1: &'static str = "00000000000000000000000000000My1";
    pub const DID_MY1: &'static str = "VsKV7grR1BUE29mG2Fm2kX";
    pub const VERKEY_MY1: &'static str = "GjZWsBLgZCR18aL468JAT7w9CZRiBnpxUPPgyQxh4voa";

    pub const SEED_MY3: &'static str = "00000000000000000000000000000My3";
    pub const DID_MY3: &'static str = "5Uu7YveFSGcT3dSzjpvPab";
    pub const VERKEY_MY3: &'static str = "3SeuRm3uYuQDYmHeuMLu1xNHozNTtzS3kbZRFMMCWrX4";

    mod did_new {
        use super::*;

        #[test]
        pub fn new_works() {
            let ctx = setup_with_wallet();
            {
                let cmd = new_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            let dids = get_dids(&ctx);
            assert_eq!(1, dids.len());

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_did() {
            let ctx = setup_with_wallet();
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did = get_did_info(&ctx, DID_TRUSTEE);
            assert_eq!(did["did"].as_str().unwrap(), DID_TRUSTEE);

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_seed() {
            let ctx = setup_with_wallet();
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("seed", SEED_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did = get_did_info(&ctx, DID_TRUSTEE);
            assert_eq!(did["did"].as_str().unwrap(), DID_TRUSTEE);
            assert_eq!(did["verkey"].as_str().unwrap(), VERKEY_TRUSTEE);

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_hex_seed() {
            let ctx = setup_with_wallet();
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "seed",
                    "94a823a6387cdd30d8f7687d95710ebab84c6e277b724790a5b221440beb7df6".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            get_did_info(&ctx, "HWvjYf77k1dqQAk6sE4gaS");

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_meta() {
            let ctx = setup_with_wallet();
            let metadata = "metadata";
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("metadata", metadata.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let dids = get_dids(&ctx);
            assert_eq!(1, dids.len());
            assert_eq!(dids[0]["metadata"].as_str().unwrap(), metadata);

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_no_opened_wallet() {
            let ctx = setup();
            {
                let cmd = new_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }

        #[test]
        pub fn new_works_for_wrong_seed() {
            let ctx = setup_with_wallet();
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("seed", "invalid_base58_string".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_method_name() {
            let ctx = setup_with_wallet();
            let method = "sov";
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("seed", SEED_TRUSTEE.to_string());
                params.insert("method", method.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let expected_did = format!("did:{}:{}", method, DID_TRUSTEE);
            let did = get_did_info(&ctx, &expected_did);
            assert_eq!(did["did"].as_str().unwrap(), &expected_did);
            assert_eq!(did["verkey"].as_str().unwrap(), VERKEY_TRUSTEE);

            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn new_works_for_not_abbreviatable() {
            let ctx = setup_with_wallet();
            let method = "indy";
            {
                let cmd = new_command::new();
                let mut params = CommandParams::new();
                params.insert("seed", SEED_TRUSTEE.to_string());
                params.insert("method", method.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let expected_did = format!("did:{}:{}", method, DID_TRUSTEE);
            let did = get_did_info(&ctx, &expected_did);
            assert_eq!(did["did"].as_str().unwrap(), &expected_did);
            assert_eq!(did["verkey"].as_str().unwrap(), VERKEY_TRUSTEE);

            tear_down_with_wallet(&ctx);
        }
    }

    mod did_use {
        use super::*;

        #[test]
        pub fn use_works() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            {
                let cmd = use_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert_eq!(ensure_active_did(&ctx).unwrap(), DID_TRUSTEE);
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn use_works_for_unknown_did() {
            let ctx = setup_with_wallet();
            {
                let cmd = use_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn use_works_for_closed_wallet() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            close_and_delete_wallet(&ctx);
            {
                let cmd = new_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }
    }

    mod did_list {
        use super::*;

        #[test]
        pub fn list_works() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn list_works_for_empty_result() {
            let ctx = setup_with_wallet();
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn list_works_for_closed_wallet() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_TRUSTEE);
            close_and_delete_wallet(&ctx);
            {
                let cmd = list_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down();
        }
    }

    mod did_rotate_key {
        use super::*;

        fn ensure_nym_written(ctx: &CommandContext, did: &str, verkey: &str) {
            let wallet_handle = ensure_opened_store(ctx).unwrap();
            let request = Ledger::build_get_nym_request(None, did).unwrap();
            let request = Ledger::sign_request(wallet_handle, did, &request).unwrap();
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

            let wallet_handle = ensure_opened_store(&ctx).unwrap();
            let (did, verkey) = Did::new(wallet_handle, "{}").unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            ensure_nym_written(&ctx, &did, &verkey);
            use_did(&ctx, &did);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), verkey);
            {
                let cmd = rotate_key_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_ne!(did_info["verkey"].as_str().unwrap(), verkey);

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn rotate_resume_works_when_ledger_updated() {
            let ctx = setup();

            let wallet_handle = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);
            let pool_handle = ensure_connected_pool_handle(&ctx).unwrap();

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::new(wallet_handle, "{}").unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            let new_verkey = Did::replace_keys_start(wallet_handle, &did, "{}").unwrap();
            let request =
                Ledger::build_nym_request(&did, &did, Some(&new_verkey), None, None).unwrap();
            Ledger::sign_and_submit_request(pool_handle, wallet_handle, &did, &request).unwrap();
            ensure_nym_written(&ctx, &did, &new_verkey);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), verkey);
            assert_eq!(did_info["tempVerkey"].as_str().unwrap(), new_verkey);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), new_verkey);
            assert_eq!(did_info["tempVerkey"].as_str(), None);

            close_and_delete_wallet(&ctx);
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn rotate_resume_works_when_ledger_not_updated() {
            let ctx = setup();

            let wallet_handle = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::new(wallet_handle, "{}").unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);
            ensure_nym_written(&ctx, &did, &verkey);

            let new_verkey = Did::replace_keys_start(wallet_handle, &did, "{}").unwrap();

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), verkey);
            assert_eq!(did_info["tempVerkey"].as_str().unwrap(), new_verkey);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), new_verkey);
            assert_eq!(did_info["tempVerkey"].as_str(), None);

            close_and_delete_wallet(&ctx);
            disconnect_and_delete_pool(&ctx);
            tear_down();
        }

        #[test]
        pub fn rotate_resume_without_started_rotation_rejected() {
            let ctx = setup();

            let wallet_handle = create_and_open_wallet(&ctx);
            create_and_connect_pool(&ctx);

            new_did(&ctx, SEED_TRUSTEE);

            let (did, verkey) = Did::new(wallet_handle, "{}").unwrap();
            use_did(&ctx, DID_TRUSTEE);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), verkey);
            assert_eq!(did_info["tempVerkey"].as_str(), None);
            {
                let cmd = rotate_key_command::new();
                let mut params = CommandParams::new();
                params.insert("resume", "true".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            let did_info = get_did_info(&ctx, &did);
            assert_eq!(did_info["verkey"].as_str().unwrap(), verkey); // it is not changed.
            assert_eq!(did_info["tempVerkey"].as_str(), None);

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

    mod qualify_did {
        use super::*;

        const METHOD: &str = "peer";

        #[test]
        pub fn qualify_did_works() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_MY1);
            {
                let cmd = qualify_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("method", METHOD.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn qualify_did_works_for_active() {
            let ctx = setup_with_wallet();
            new_did(&ctx, SEED_MY1);
            use_did(&ctx, DID_MY1);
            {
                let cmd = qualify_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("method", METHOD.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet(&ctx);
        }

        #[test]
        pub fn qualify_did_works_for_unknown_did() {
            let ctx = setup_with_wallet();
            {
                let cmd = qualify_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY1.to_string());
                params.insert("method", METHOD.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet(&ctx);
        }
    }

    fn get_did_info(ctx: &CommandContext, did: &str) -> serde_json::Value {
        let wallet_handle = ensure_opened_store(ctx).unwrap();
        let did_info = Did::get_did_with_meta(wallet_handle, did).unwrap();
        serde_json::from_str(&did_info).unwrap()
    }

    fn get_dids(ctx: &CommandContext) -> Vec<serde_json::Value> {
        let wallet_handle = ensure_opened_store(ctx).unwrap();
        let dids = Did::list_dids_with_meta(wallet_handle).unwrap();
        serde_json::from_str(&dids).unwrap()
    }

    pub fn new_did(ctx: &CommandContext, seed: &str) {
        {
            let cmd = new_command::new();
            let mut params = CommandParams::new();
            params.insert("seed", seed.to_string());
            cmd.execute(&ctx, &params).unwrap();
        }
    }

    pub fn use_did(ctx: &CommandContext, did: &str) {
        {
            let cmd = use_command::new();
            let mut params = CommandParams::new();
            params.insert("did", did.to_string());
            cmd.execute(&ctx, &params).unwrap();
        }
    }
}
