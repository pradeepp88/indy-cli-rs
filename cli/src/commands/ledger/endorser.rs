/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{parse_transaction_response, Ledger, Response},
    utils::table::print_table,
};

use super::common::handle_transaction_response;
use indy_vdr::pool::PreparedRequest;
use serde_json::Value as JsonValue;

pub mod endorse_transaction_command {
    use super::*;

    command!(CommandMetadata::build(
        "endorse",
        "Endorse transaction to the ledger preserving an original author."
    )
    .add_optional_param(
        "txn",
        "Transaction to endorse. Skip to use a transaction stored into CLI context."
    )
    .add_example(r#"ledger endorse txn={"reqId":123456789,"type":"100"}"#)
    .add_example(r#"ledger endorse"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let wallet_name = ctx.ensure_opened_wallet_name()?;
        let submitter_did = ctx.ensure_active_did()?;

        let param_txn = ParamParser::get_opt_str_param("txn", params).map_err(error_err!())?;

        let mut request = get_transaction_to_use!(ctx, param_txn);

        Ledger::multi_sign_request(&wallet, &submitter_did, &mut request)
            .map_err(|err| println_err!("{}", err.message(Some(&wallet_name))))?;

        let (_, response) = send_request!(&ctx, params, &request, None, Some(&submitter_did), true);

        let (metadata_headers, metadata, data) = handle_transaction_response(response)
            .and_then(|result| parse_transaction_response(result))?;

        println_succ!("Transaction has been sent to Ledger.");

        println_succ!("Metadata:");
        print_table(&metadata, &metadata_headers);

        println_succ!("Data:");
        print_table(&json!({ "data": data }), &[("data", "Data")]);

        trace!("execute <<");
        Ok(())
    }
}
