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

use super::common::{handle_transaction_response, print_transaction_response};

pub mod pool_upgrade_command {
    use super::*;

    command!(CommandMetadata::build("pool-upgrade", "Send instructions to nodes to update themselves.")
                .add_required_param("name", "Human-readable name for the upgrade.")
                .add_required_param("version","The version of indy-node package we perform upgrade to. \n                  \
                                              Must be greater than existing one (or equal if reinstall flag is True)")
                .add_required_param("action", "Upgrade type. Either start or cancel.")
                .add_required_param("sha256", "Sha256 hash of the package.")
                .add_optional_param("timeout", "Limits upgrade time on each Node.")
                .add_optional_param("schedule", "Node upgrade schedule. Schedule should contain identifiers of all nodes. Upgrade dates should be in future. \n                              \
                                              If force flag is False, then it's required that time difference between each Upgrade must be not less than 5 minutes.\n                              \
                                              Requirements for schedule can be ignored by parameter force=true.\n                              \
                                              Schedule is mandatory for action=start.")
                .add_optional_param("justification", "Justification string for this particular Upgrade.")
                .add_optional_param("reinstall", "Whether it's allowed to re-install the same version. False by default.")
                .add_optional_param("force", "Whether we should apply transaction without waiting for consensus of this transaction. False by default.")
                .add_optional_param("package", "Package to be upgraded.")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=start sha256=f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398 schedule={"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00"}"#)
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=start sha256=f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398 schedule={"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00"} package=some_package"#)
                .add_example(r#"ledger pool-upgrade name=upgrade-1 version=2.0 action=cancel sha256=ac3eb2cc3ac9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let name = ParamParser::get_str_param("name", params)?;
        let version = ParamParser::get_str_param("version", params)?;
        let action = ParamParser::get_str_param("action", params)?;
        let sha256 = ParamParser::get_str_param("sha256", params)?;
        let timeout = ParamParser::get_opt_number_param::<u32>("timeout", params)?;
        let schedule = ParamParser::get_opt_str_param("schedule", params)?;
        let justification = ParamParser::get_opt_str_param("justification", params)?;
        let reinstall = ParamParser::get_opt_bool_param("reinstall", params)?.unwrap_or(false);
        let force = ParamParser::get_opt_bool_param("force", params)?.unwrap_or(false);
        let package = ParamParser::get_opt_str_param("package", params)?;

        let mut request = Ledger::indy_build_pool_upgrade_request(
            pool.as_deref(),
            &submitter_did,
            name,
            version,
            action,
            sha256,
            timeout,
            schedule,
            justification,
            reinstall,
            force,
            package,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        let mut schedule = None;
        let mut hash = None;
        if let Some(res) = response.result.as_ref() {
            schedule = res["schedule"].as_object().map(|s| {
                format!(
                    "{{{}\n}}",
                    s.iter()
                        .map(|(key, value)| format!(
                            "\n    {:?}:{:?}",
                            key,
                            value.as_str().unwrap_or("")
                        ))
                        .collect::<Vec<String>>()
                        .join(",")
                )
            });

            hash = res["sha256"].as_str().map(|h| h.to_string());
        };

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                None,
                &[
                    ("name", "Name"),
                    ("action", "Action"),
                    ("version", "Version"),
                    ("timeout", "Timeout"),
                    ("justification", "Justification"),
                    ("reinstall", "Reinstall"),
                    ("force", "Force Apply"),
                    ("package", "Package Name"),
                ],
                true,
            )
        })?;
        if let Some(h) = hash {
            println_succ!("Hash:");
            println!("{}", h);
        }
        if let Some(s) = schedule {
            println_succ!("Schedule:");
            println!("{}", s);
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
        ledger::tests::use_trustee,
    };

    mod pool_upgrade {
        use super::*;

        #[test]
        #[ignore]
        pub fn pool_upgrade_works() {
            let schedule = r#"{"Gw6pDLhcBcoQesN72qfotTgFa7cbuqZpkX3Xo6pLhPhv":"2020-01-25T12:49:05.258870+00:00",
                                    "8ECVSk179mjsjKRLWiQtssMLgp6EPhWXtaYyStWPSGAb":"2020-01-25T13:49:05.258870+00:00",
                                    "DKVxG2fXXTU8yT5N7hGEbXB3dfdAnYv1JczDUHpmDxya":"2020-01-25T14:49:05.258870+00:00",
                                    "4PS3EDQ3dW1tci1Bp6543CfuuebjFrg36kLAUcskGfaA":"2020-01-25T15:49:05.258870+00:00"}"#;

            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = pool_upgrade_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "upgrade-indy-cli".to_string());
                params.insert("version", "2.0.0".to_string());
                params.insert("action", "start".to_string());
                params.insert(
                    "sha256",
                    "f284bdc3c1c9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398".to_string(),
                );
                params.insert("schedule", schedule.to_string());
                params.insert("force", "true".to_string()); // because node_works test added fifth Node
                cmd.execute(&ctx, &params).unwrap();
            }
            // There is no way to read upgrade transaction to be sure about completely write before sending next one.
            // So just sleep agains other places where control read request is available
            ::std::thread::sleep(::std::time::Duration::from_secs(1));
            {
                let cmd = pool_upgrade_command::new();
                let mut params = CommandParams::new();
                params.insert("name", "upgrade-indy-cli".to_string());
                params.insert("version", "2.0.0".to_string());
                params.insert("action", "cancel".to_string());
                params.insert(
                    "sha256",
                    "ac3eb2cc3ac9e24a494e285cb387c69510f28de51c15bb93179d9c7f28705398".to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
