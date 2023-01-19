/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, Response},
    utils::{file::read_file, table::print_list_table},
};

use serde_json::Value as JsonValue;

use super::common::{handle_transaction_response, print_transaction_response};

pub mod taa_command {
    use super::*;

    command!(CommandMetadata::build("txn-author-agreement", r#"Send Transaction Author Agreement to the ledger."#)
                .add_optional_param("text", r#"The content of a new agreement.
                         Mandatory in case of adding a new TAA. An existing TAA text can not be changed.
                         for Indy Node version <= 1.12.0:
                             Use empty string to reset TAA on the ledger
                         for Indy Node version > 1.12.0
                             Should be omitted in case of updating an existing TAA (setting `retirement-timestamp`)
                "#)
                .add_optional_param("file", "The path to file containing a content of agreement to send (an alternative to the `text` parameter)")
                .add_required_param("version", "The version of a new agreement")
                .add_optional_param("ratification-timestamp",r#"The date (timestamp) of TAA ratification by network government
                                 for Indy Node version <= 1.12.0:
                                    Must be omitted
                                 for Indy Node version > 1.12.0:
                                    Must be specified in case of adding a new TAA
                                    Can be omitted in case of updating an existing TAA
                "#)
                .add_optional_param("retirement-timestamp", r#"The date (timestamp) of TAA retirement.
                                for Indy Node version <= 1.12.0:
                                    Must be omitted
                                for Indy Node version > 1.12.0:
                                    Must be omitted in case of adding a new (latest) TAA.
                                    Should be used for updating (deactivating) non-latest TAA on the ledger.
                "#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger txn-author-agreement text=\"Indy transaction agreement\" version=1")
                .add_example("ledger txn-author-agreement text= version=1")
                .add_example("ledger txn-author-agreement file=/home/agreement_content.txt version=1")
                .add_example("ledger txn-author-agreement text=\"Indy transaction agreement\" version=1 send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let text = ParamParser::get_opt_empty_str_param("text", params)?;
        let file = ParamParser::get_opt_str_param("file", params)?;
        let version = ParamParser::get_str_param("version", params)?;
        let ratification_ts =
            ParamParser::get_opt_number_param::<u64>("ratification-timestamp", params)?;
        let retirement_ts =
            ParamParser::get_opt_number_param::<u64>("retirement-timestamp", params)?;

        let text: Option<String> = match (text, file) {
            (Some(text_), None) => Some(text_.to_string()),
            (None, Some(file_)) => Some(read_file(file_).map_err(|err| println_err!("{}", err))?),
            (Some(_), Some(_)) => {
                println_err!("Only one of the parameters `text` and `file` can be specified");
                return Err(());
            }
            (None, None) => None,
        };

        let mut request = Ledger::build_txn_author_agreement_request(
            pool.as_deref(),
            &submitter_did,
            text.as_ref().map(String::as_str),
            &version,
            ratification_ts,
            retirement_ts,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        handle_transaction_response(response).map(|result| {
            // TODO support multiply active TAA on the ledger IS-1441
            if let Some(text) = text {
                print_transaction_response(
                    result,
                    "Transaction Author Agreement has been sent to Ledger.",
                    None,
                    &[
                        ("text", "Text"),
                        ("version", "Version"),
                        ("ratification_ts", "Ratification Time"),
                        ("retirement_ts", "Retirement Time"),
                    ],
                    true,
                );
                crate::commands::pool::accept_transaction_author_agreement(ctx, &text, &version);
            }
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod taa_disable_all_command {
    use super::*;

    command!(CommandMetadata::build("disable-all-txn-author-agreements", r#"Disable All Transaction Author Agreements on the ledger"#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger disable-all-txn-author-agreements")
                .add_example("ledger disable-all-txn-author-agreements send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let mut request = Ledger::build_disable_all_txn_author_agreements_request(
            pool.as_deref(),
            &submitter_did,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        handle_transaction_response(response).map(|_| {
            ctx.set_transaction_author_info(None);
            println_succ!("All Transaction Author Agreements on the Ledger have been disabled");
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod aml_command {
    use super::*;

    command!(CommandMetadata::build("txn-acceptance-mechanisms", r#"Send TAA Acceptance Mechanisms to the ledger."#)
                .add_optional_param("aml", "The set of new acceptance mechanisms.")
                .add_optional_param("file", "The path to file containing a set of acceptance mechanisms to send (an alternative to the text parameter).")
                .add_required_param("version", "The version of a new set of acceptance mechanisms.")
                .add_optional_param("context", "Common context information about acceptance mechanisms (may be a URL to external resource).")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1")
                .add_example("ledger txn-acceptance-mechanisms file=/home/mechanism.txt version=1")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1 context=\"some context\"")
                .add_example("ledger txn-acceptance-mechanisms aml={\"Click Agreement\":\"some description\"} version=1 send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let aml = ParamParser::get_opt_str_param("aml", params)?;
        let file = ParamParser::get_opt_str_param("file", params)?;
        let version = ParamParser::get_str_param("version", params)?;
        let context = ParamParser::get_opt_str_param("context", params)?;

        let aml = match (aml, file) {
            (Some(aml_), None) => aml_.to_string(),
            (None, Some(file_)) => read_file(file_).map_err(|err| println_err!("{}", err))?,
            (Some(_), Some(_)) => {
                println_err!("Only one of the parameters `aml` and `file` can be specified");
                return Err(());
            }
            (None, None) => {
                println_err!("Either `aml` or `file` parameter must be specified");
                return Err(());
            }
        };

        let mut request = Ledger::build_acceptance_mechanisms_request(
            pool.as_deref(),
            &submitter_did,
            &aml,
            &version,
            context,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Acceptance Mechanisms have been sent to Ledger.",
                None,
                &[
                    ("aml", "Text"),
                    ("version", "Version"),
                    ("amlContext", "Context"),
                ],
                true,
            )
        })?;

        trace!("execute <<");
        Ok(())
    }
}

pub mod get_acceptance_mechanisms_command {
    use super::*;

    command!(CommandMetadata::build("get-acceptance-mechanisms", r#"Get a list of acceptance mechanisms set on the ledger"#)
                .add_optional_param("timestamp","The time (as timestamp) to get an active acceptance mechanisms. Skip to get the latest one")
                .add_optional_param("version","The version of acceptance mechanisms")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger get-acceptance-mechanisms")
                .add_example("ledger get-acceptance-mechanisms timestamp=1576674598")
                .add_example("ledger get-acceptance-mechanisms version=1.0")
                .add_example("ledger get-acceptance-mechanisms send=false")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = ctx.get_active_did()?;
        let pool = ctx.get_connected_pool();

        let timestamp = ParamParser::get_opt_number_param::<u64>("timestamp", params)?;
        let version = ParamParser::get_opt_str_param("version", params)?;

        let request = Ledger::build_get_acceptance_mechanisms_request(
            pool.as_deref(),
            submitter_did.as_deref(),
            timestamp,
            version,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request);

        match handle_transaction_response(response) {
            Ok(result) => {
                let aml = result["data"]["aml"]
                    .as_object()
                    .ok_or_else(|| println_err!("Wrong data has been received"))?;

                let aml = aml
                    .iter()
                    .map(|(key, value)| {
                        json!({
                            "label": key,
                            "description": value
                        })
                    })
                    .collect::<Vec<JsonValue>>();

                if !aml.is_empty() {
                    println!("Following Acceptance Mechanisms are set on the Ledger");
                }

                print_list_table(
                    &aml,
                    &[("label", "Label"), ("description", "Description")],
                    "There are no acceptance mechanisms",
                );

                println!(
                    "Version: {}",
                    result["data"]["version"].as_str().unwrap_or_default()
                );

                if let Some(context) = result["data"]["amlContext"].as_str() {
                    println!("Context: {}", context);
                }
                println!();
            }
            Err(_) => {}
        }

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::{
            nym_command,
            tests::{create_new_did, use_trustee},
        },
    };

    mod aml {
        use super::*;
        use crate::ledger::tests::use_trustee;
        use chrono::Utc;

        pub const AML: &str = r#"{"Acceptance Mechanism 1": "Description 1", "Acceptance Mechanism 2": "Description 2"}"#;

        pub fn _get_version() -> String {
            Utc::now().timestamp().to_string()
        }

        #[test]
        pub fn acceptance_mechanisms_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = aml_command::new();
                let mut params = CommandParams::new();
                params.insert("aml", AML.to_string());
                params.insert("version", _get_version());
                params.insert("context", "Some Context".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            ::std::thread::sleep(::std::time::Duration::from_secs(1));
            {
                let cmd = get_acceptance_mechanisms_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }

    mod taa {
        use super::*;

        #[test]
        pub fn taa_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                // Set AML
                let cmd = aml_command::new();
                let mut params = CommandParams::new();
                params.insert("aml", super::aml::AML.to_string());
                params.insert("version", super::aml::_get_version());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Set TAA
                let cmd = taa_command::new();
                let mut params = CommandParams::new();
                params.insert("text", "test taa".to_string());
                params.insert("version", super::aml::_get_version());
                params.insert("ratification-timestamp", "123456789".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Send Nym
                let (did, _) = create_new_did(&ctx);
                ctx.set_taa_acceptance_mechanism("Acceptance Mechanism 1");
                let cmd = nym_command::new();
                let mut params = CommandParams::new();
                params.insert("did", did);
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                // Disable all TAAs
                let cmd = taa_disable_all_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
