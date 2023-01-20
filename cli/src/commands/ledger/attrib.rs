/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response},
};

use serde_json::Value as JsonValue;

use super::common::{
    handle_transaction_response, print_transaction_response, set_author_agreement,
};

pub mod attrib_command {
    use super::*;

    command!(CommandMetadata::build("attrib", r#"Send Attribute transaction to the Ledger for exists NYM."#)
                .add_required_param("did",  "DID of identity presented in Ledger")
                .add_optional_param("hash", "Hash of attribute data")
                .add_optional_param("raw", "JSON representation of attribute data")
                .add_optional_param("enc", "Encrypted attribute data")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_optional_param("endorser","DID of the Endorser that will submit the transaction to the ledger later. \
                    Note that specifying of this parameter implies send=false so the transaction will be prepared to pass to the endorser instead of sending to the ledger.\
                    The created request will be printed and stored into CLI context.")
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX raw={"endpoint":{"ha":"127.0.0.1:5555"}}"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX hash=83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX enc=aa3f41f619aa7e5e6b6d0d"#)
                .add_example(r#"ledger attrib did=VsKV7grR1BUE29mG2Fm2kX raw={"endpoint":{"ha":"127.0.0.1:5555"}} send=false"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let pool = ctx.get_connected_pool();
        let submitter_did = ctx.ensure_active_did()?;

        let target_did = ParamParser::get_did_param("did", params)?;
        let hash = ParamParser::get_opt_str_param("hash", params)?;
        let raw = ParamParser::get_opt_object_param("raw", params)?;
        let enc = ParamParser::get_opt_str_param("enc", params)?;

        let mut request = Ledger::build_attrib_request(
            pool.as_deref(),
            &submitter_did,
            &target_did,
            hash,
            raw.as_ref(),
            enc,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        set_author_agreement(ctx, &mut request)?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        let attribute = if raw.is_some() {
            ("raw", "Raw value")
        } else if hash.is_some() {
            ("hash", "Hashed value")
        } else {
            ("enc", "Encrypted value")
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Attrib request has been sent to Ledger.",
                None,
                &[attribute],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_attrib_command {
    use super::*;

    command!(CommandMetadata::build("get-attrib", "Get ATTRIB from Ledger.")
                .add_required_param("did", "DID of identity presented in Ledger")
                .add_optional_param("raw", "Name of attribute")
                .add_optional_param("hash", "Hash of attribute data")
                .add_optional_param("enc", "Encrypted value of attribute data")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX raw=endpoint")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX hash=83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3")
                .add_example("ledger get-attrib did=VsKV7grR1BUE29mG2Fm2kX enc=aa3f41f619aa7e5e6b6d0d")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = ctx.get_active_did()?;
        let pool = ctx.get_connected_pool();

        let target_did = ParamParser::get_did_param("did", params)?;
        let raw = ParamParser::get_opt_str_param("raw", params)?;
        let hash = ParamParser::get_opt_str_param("hash", params)?;
        let enc = ParamParser::get_opt_str_param("enc", params)?;

        let request = Ledger::build_get_attrib_request(
            pool.as_deref(),
            submitter_did.as_deref(),
            &target_did,
            raw,
            hash,
            enc,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response) = send_read_request!(&ctx, params, &request);

        if let Some(result) = response.result.as_mut() {
            let data = result["data"]
                .as_str()
                .map(|data| JsonValue::String(data.to_string()));
            match data {
                Some(data) => {
                    result["data"] = data;
                }
                None => {
                    println_err!("Attribute not found");
                    return Err(());
                }
            };
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Following ATTRIB has been received.",
                None,
                &[("data", "Data")],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{
            did::tests::{new_did, use_did, DID_MY3, DID_TRUSTEE, SEED_MY3},
            setup_with_wallet_and_pool, submit_retry, tear_down_with_wallet_and_pool,
            wallet::tests::{close_wallet, open_wallet},
        },
        ledger::{
            endorse_transaction_command,
            tests::{create_new_did, send_nym, use_new_endorser, use_trustee, ReplyResult},
        },
    };
    use indy_utils::did::DidValue;

    const ATTRIB_RAW_DATA: &str = r#"{"endpoint":{"ha":"127.0.0.1:5555"}}"#;
    const ATTRIB_HASH_DATA: &str =
        r#"83d907821df1c87db829e96569a11f6fc2e7880acba5e43d07ab786959e13bd3"#;
    const ATTRIB_ENC_DATA: &str = r#"aa3f41f619aa7e5e6b6d0d"#;

    mod attrib {
        use super::*;

        #[test]
        pub fn attrib_works_for_raw_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_hash_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, None, Some(ATTRIB_HASH_DATA), None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_enc_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, None, None, Some(ATTRIB_ENC_DATA)).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_missed_attribute() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_unknown_did() {
            let ctx = setup_with_wallet_and_pool();

            new_did(&ctx, SEED_MY3);
            use_did(&ctx, DID_MY3);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_MY3.to_string());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_invalid_endpoint_format() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", DID_TRUSTEE.to_string());
                params.insert("raw", r#"127.0.0.1:5555"#.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_raw_value_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_err());
            assert!(ctx.get_context_transaction().is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_without_signing() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("sign", "false".to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            let transaction = ctx.get_context_transaction().unwrap();
            let transaction: JsonValue = serde_json::from_str(&transaction).unwrap();
            assert!(transaction["signature"].is_null());
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn attrib_works_for_endorser() {
            let ctx = setup_with_wallet_and_pool();
            let (endorser_did, _) = use_new_endorser(&ctx);

            // Publish new NYM without any role
            let (did, verkey) = create_new_did(&ctx);
            send_nym(&ctx, &did, &verkey, None);
            use_did(&ctx, &did);

            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                params.insert("endorser", endorser_did.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            use_did(&ctx, &endorser_did);
            {
                let cmd = endorse_transaction_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod get_attrib {
        use super::*;

        #[test]
        pub fn get_attrib_works_for_raw_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", "endpoint".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_hash_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, None, Some(ATTRIB_HASH_DATA), None).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("hash", ATTRIB_HASH_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_enc_value() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, None, None, Some(ATTRIB_ENC_DATA)).is_ok());
            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("enc", ATTRIB_ENC_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_attrib_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            let (did, _) = use_new_endorser(&ctx);
            {
                let cmd = attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", ATTRIB_RAW_DATA.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ensure_attrib_added(&ctx, &did, Some(ATTRIB_RAW_DATA), None, None).is_ok());

            // to reset active did
            close_wallet(&ctx);
            open_wallet(&ctx);

            {
                let cmd = get_attrib_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did.clone());
                params.insert("raw", "endpoint".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    pub fn ensure_attrib_added(
        ctx: &CommandContext,
        did: &str,
        raw: Option<&str>,
        hash: Option<&str>,
        enc: Option<&str>,
    ) -> Result<(), ()> {
        let pool = ctx.get_connected_pool().unwrap();
        let attr = if raw.is_some() {
            Some("endpoint")
        } else {
            None
        };
        let did = DidValue(did.to_string());
        let request =
            Ledger::build_get_attrib_request(Some(&pool), None, &did, attr, hash, enc).unwrap();
        submit_retry(ctx, &request, |response| {
            serde_json::from_str::<Response<ReplyResult<String>>>(&response)
                .map_err(|_| ())
                .and_then(|response| {
                    let expected_value = if raw.is_some() {
                        raw.unwrap()
                    } else if hash.is_some() {
                        hash.unwrap()
                    } else {
                        enc.unwrap()
                    };
                    if response.result.is_some() && expected_value == response.result.unwrap().data
                    {
                        Ok(())
                    } else {
                        Err(())
                    }
                })
        })
    }
}
