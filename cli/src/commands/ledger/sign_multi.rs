/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::Ledger,
};

use indy_vdr::pool::PreparedRequest;

pub mod sign_multi_command {
    use super::*;
    use crate::error::CliError;
    use indy_vdr::common::error::VdrErrorKind;

    command!(CommandMetadata::build(
        "sign-multi",
        "Add multi signature by current DID to transaction."
    )
    .add_optional_param(
        "txn",
        "Transaction to sign. Skip to use a transaction stored into CLI context."
    )
    .add_example(r#"ledger sign-multi txn={"reqId":123456789,"type":"100"}"#)
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;

        let param_txn = ParamParser::get_opt_str_param("txn", params)?;

        let mut txn = get_transaction_to_use!(ctx, param_txn);

        match Ledger::multi_sign_request(&wallet, &submitter_did, &mut txn) {
            Ok(_) => {
                println_succ!("Transaction has been signed:");
                println_succ!("{:?}", txn.req_json.to_string());
                ctx.set_context_transaction(Some(txn.req_json.to_string()));
            }
            Err(err) => match err {
                CliError::VdrError(ref vdr_err) => match vdr_err.kind() {
                    VdrErrorKind::Unexpected => {
                        println_err!("Signer DID: \"{}\" not found", submitter_did);
                    }
                    _ => {
                        println_err!("{}", err.message(None));
                    }
                },
                _ => {
                    println_err!("{}", err.message(None));
                }
            },
        };

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::tests::{use_trustee, TRANSACTION},
    };

    mod sign_multi {
        use super::*;

        #[test]
        pub fn sign_multi_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn sign_multi_works_for_no_active_did() {
            let ctx = setup_with_wallet_and_pool();
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", TRANSACTION.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn sign_multi_works_for_invalid_message_format() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = sign_multi_command::new();
                let mut params = CommandParams::new();
                params.insert("txn", r#"1496822211362017764"#.to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
