/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::ledger::{Ledger, LedgerHelpers, Response},
    utils::table::print_list_table,
};

use serde_json::Value as JsonValue;

use super::common::{handle_transaction_response, print_transaction_response};

#[derive(Deserialize, Debug)]
pub struct AuthRuleData {
    pub auth_type: String,
    pub auth_action: String,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub constraint: JsonValue,
}

pub type AuthRulesData = Vec<AuthRuleData>;

pub mod auth_rule_command {
    use super::*;
    use indy_vdr::ledger::constants::txn_name_to_code;

    command!(CommandMetadata::build("auth-rule", "Send AUTH_RULE request to change authentication rules for a ledger transaction.")
                .add_required_param("txn_type", "Ledger transaction alias or associated value")
                .add_required_param("action", "Type of an action. One of: ADD, EDIT")
                .add_required_param("field", "Transaction field")
                .add_optional_param("old_value", "Old value of field, which can be changed to a new_value (mandatory for EDIT action)")
                .add_optional_param("new_value", "New value that can be used to fill the field")
                .add_required_param("constraint", r#"Set of constraints required for execution of an action
         {
             constraint_id - type of a constraint. Can be either "ROLE" to specify final constraint or  "AND"/"OR" to combine constraints, or "FORBIDDEN" to forbid action.
             role - (optional) role associated value {TRUSTEE: 0, STEWARD: 2, TRUST_ANCHOR: 101, ENDORSER: 101, NETWORK_MONITOR: 201, ANY: *}.
             sig_count - the number of signatures required to execution action.
             need_to_be_owner - (optional) if user must be an owner of transaction (false by default).
             off_ledger_signature - (optional) allow signature of unknow for ledger did (false by default).
             metadata - (optional) additional parameters of the constraint.
         }
         can be combined by
         {
             constraint_id: <"AND" or "OR">
             auth_constraints: [<constraint_1>, <constraint_2>]
         }
                "#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false,"off_ledger_signature":true}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=EDIT field=role old_value=101 new_value=0 constraint="{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}""#)
                .add_example(r#"ledger auth-rule txn_type=NYM action=ADD field=role new_value=101 constraint="{"constraint_id":"FORBIDDEN"}""#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let txn_type = ParamParser::get_str_param("txn_type", params)?;
        let action = ParamParser::get_str_param("action", params)?;
        let field = ParamParser::get_str_param("field", params)?;
        let old_value = ParamParser::get_opt_str_param("old_value", params)?;
        let new_value = ParamParser::get_opt_str_param("new_value", params)?;
        let constraint = ParamParser::get_str_param("constraint", params)?;

        let txn_type = txn_name_to_code(txn_type)
            .ok_or_else(|| println_err!("Unsupported ledger transaction."))?;

        let mut request = Ledger::build_auth_rule_request(
            pool.as_deref(),
            &submitter_did,
            &txn_type,
            &action.to_uppercase(),
            field,
            old_value,
            new_value,
            constraint,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, mut response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        if let Some(result) = response.result.as_mut() {
            result["txn"]["data"]["auth_type"] =
                LedgerHelpers::get_txn_title(&result["txn"]["data"]["auth_type"]);
            result["txn"]["data"]["constraint"] = JsonValue::String(
                serde_json::to_string_pretty(&result["txn"]["data"]["constraint"]).unwrap(),
            );
        }

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "Auth Rule request has been sent to Ledger.",
                None,
                &[
                    ("auth_type", "Txn Type"),
                    ("auth_action", "Action"),
                    ("field", "Field"),
                    ("old_value", "Old Value"),
                    ("new_value", "New Value"),
                    ("constraint", "Constraint"),
                ],
                false,
            )
        })?;

        trace!("execute << ");
        Ok(())
    }
}

pub mod auth_rules_command {
    use super::*;

    command!(CommandMetadata::build("auth-rules", "Send AUTH_RULES request to change authentication rules for multiple ledger transactions.")
                .add_main_param("rules", r#"A list of auth rules: [{"auth_type", "auth_action", "field", "old_value", "new_value"},{...}]"#)
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger auth-rules [{"auth_type":"1","auth_action":"ADD","field":"role","new_value":"101","constraint":{"sig_count":1,"role":"0","constraint_id":"ROLE","need_to_be_owner":false}}]"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let rules = ParamParser::get_str_param("rules", params)?;

        let mut request = Ledger::build_auth_rules_request(pool.as_deref(), &submitter_did, &rules)
            .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        let result = handle_transaction_response(response)?;
        println!("result {:?}", result);

        let rules: AuthRulesData = serde_json::from_value(result["txn"]["data"]["rules"].clone())
            .map_err(|_| println_err!("Wrong data has been received"))?;
        print_auth_rules(rules);

        trace!("execute << ");
        Ok(())
    }
}

pub mod get_auth_rule_command {
    use super::*;

    command!(CommandMetadata::build("get-auth-rule", r#"Send GET_AUTH_RULE request to get authentication rules for ledger transactions.
        Note: Either none or all parameters must be specified (`old_value` can be skipped for `ADD` action)."#)
                .add_required_param("txn_type", "Ledger transaction alias or associated value.")
                .add_required_param("action", "Type of action for. One of: ADD, EDIT")
                .add_required_param("field", "Transaction field")
                .add_optional_param("old_value", "Old value of field, which can be changed to a new_value (mandatory for EDIT action)")
                .add_required_param("new_value", "New value that can be used to fill the field")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example(r#"ledger get-auth-rule txn_type=NYM action=ADD field=role new_value=101"#)
                .add_example(r#"ledger get-auth-rule txn_type=NYM action=EDIT field=role old_value=101 new_value=0"#)
                .add_example(r#"ledger get-auth-rule"#)
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let submitter_did = ctx.get_active_did()?;
        let pool = ctx.get_connected_pool();

        let auth_type = ParamParser::get_opt_str_param("txn_type", params)?;
        let auth_action = ParamParser::get_opt_str_param("action", params)?;
        let field = ParamParser::get_opt_str_param("field", params)?;
        let old_value = ParamParser::get_opt_str_param("old_value", params)?;
        let new_value = ParamParser::get_opt_str_param("new_value", params)?;

        let request = Ledger::build_get_auth_rule_request(
            pool.as_deref(),
            submitter_did.as_deref(),
            auth_type,
            auth_action,
            field,
            old_value,
            new_value,
        )
        .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response) = send_read_request!(&ctx, params, &request);

        let result = handle_transaction_response(response)?;

        let rules: AuthRulesData = serde_json::from_value(result["data"].clone())
            .map_err(|_| println_err!("Wrong data has been received"))?;

        print_auth_rules(rules);

        trace!("execute << ");
        Ok(())
    }
}

fn print_auth_rules(rules: AuthRulesData) {
    let constraints = rules
        .into_iter()
        .map(|rule| {
            let auth_type =
                LedgerHelpers::get_txn_title(&JsonValue::String(rule.auth_type.clone()));
            let action = rule.auth_action;
            let field = rule.field;
            let old_value = if action == "ADD" {
                None
            } else {
                rule.old_value
            };
            let new_value = rule.new_value;

            json!({
                "auth_type": auth_type,
                "auth_action": action,
                "field": field,
                "old_value": old_value,
                "new_value": new_value,
                "constraint": ::serde_json::to_string_pretty(&rule.constraint).unwrap(),
            })
        })
        .collect::<Vec<JsonValue>>();

    print_list_table(
        &constraints,
        &[
            ("auth_type", "Type"),
            ("auth_action", "Action"),
            ("field", "Field"),
            ("old_value", "Old Value"),
            ("new_value", "New Value"),
            ("constraint", "Constraint"),
        ],
        "There are no rules set",
    );
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup_with_wallet_and_pool, tear_down_with_wallet_and_pool},
        ledger::tests::use_trustee,
    };

    mod auth_rule {
        use super::*;

        const AUTH_TYPE: &str = "NYM";
        const AUTH_ACTION: &str = "ADD";
        const FIELD: &str = "role";
        const NEW_VALUE: &str = "101";
        const ROLE_CONSTRAINT: &str = r#"{
            "sig_count": 1,
            "metadata": {},
            "role": "0",
            "constraint_id": "ROLE",
            "need_to_be_owner": false
        }"#;

        #[test]
        pub fn auth_rule_works_for_adding_new_trustee() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "ADD".to_string());
                params.insert("field", "role".to_string());
                params.insert("new_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn auth_rule_works_for_demoting_trustee() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "EDIT".to_string());
                params.insert("field", "role".to_string());
                params.insert("old_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_one_constraint() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", FIELD.to_string());
                params.insert("new_value", NEW_VALUE.to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            {
                let cmd = get_auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", FIELD.to_string());
                params.insert("new_value", NEW_VALUE.to_string());
                cmd.execute(&ctx, &params).unwrap();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_get_all() {
            let ctx = setup_with_wallet_and_pool();

            {
                let cmd = get_auth_rule_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn get_auth_rule_works_for_no_constraint() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = get_auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", AUTH_TYPE.to_string());
                params.insert("action", AUTH_ACTION.to_string());
                params.insert("field", "WRONG_FIELD".to_string());
                params.insert("new_value", "WRONG_VALUE".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down_with_wallet_and_pool(&ctx);
        }

        #[test]
        pub fn auth_rule_without_sending() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            {
                let cmd = auth_rule_command::new();
                let mut params = CommandParams::new();
                params.insert("txn_type", "NYM".to_string());
                params.insert("action", "ADD".to_string());
                params.insert("field", "role".to_string());
                params.insert("new_value", "0".to_string());
                params.insert("constraint", ROLE_CONSTRAINT.to_string());
                params.insert("send", "false".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            assert!(ctx.get_context_transaction().is_some());
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
