/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use indy_vdr::ledger::constants::*;
use serde_json::Value as JsonValue;

pub struct LedgerHelpers;

impl LedgerHelpers {
    pub fn get_role_title(role: &JsonValue) -> JsonValue {
        JsonValue::String(
            match role.as_str() {
                Some(TRUSTEE) => "TRUSTEE",
                Some(STEWARD) => "STEWARD",
                Some(ENDORSER) => "ENDORSER",
                Some(NETWORK_MONITOR) => "NETWORK_MONITOR",
                _ => "-",
            }
            .to_string(),
        )
    }

    pub fn get_txn_title(txn_type: &JsonValue) -> JsonValue {
        JsonValue::String(
            match txn_type.as_str() {
                Some(NODE) => "NODE",
                Some(NYM) => "NYM",
                Some(GET_TXN) => "GET_TXN",
                Some(TXN_AUTHR_AGRMT) => "TXN_AUTHR_AGRMT",
                Some(TXN_AUTHR_AGRMT_AML) => "TXN_AUTHR_AGRMT_AML",
                Some(GET_TXN_AUTHR_AGRMT) => "GET_TXN_AUTHR_AGRMT",
                Some(GET_TXN_AUTHR_AGRMT_AML) => "GET_TXN_AUTHR_AGRMT_AML",
                Some(LEDGERS_FREEZE) => "LEDGERS_FREEZE",
                Some(GET_FROZEN_LEDGERS) => "GET_FROZEN_LEDGERS",
                Some(ATTRIB) => "ATTRIB",
                Some(SCHEMA) => "SCHEMA",
                Some(GET_ATTR) => "GET_ATTR",
                Some(GET_NYM) => "GET_NYM",
                Some(GET_SCHEMA) => "GET_SCHEMA",
                Some(GET_CRED_DEF) => "GET_CRED_DEF",
                Some(CRED_DEF) => "CRED_DEF",
                Some(POOL_UPGRADE) => "POOL_UPGRADE",
                Some(POOL_CONFIG) => "POOL_CONFIG",
                Some(REVOC_REG_DEF) => "REVOC_REG_DEF",
                Some(REVOC_REG_ENTRY) => "REVOC_REG_ENTRY",
                Some(GET_REVOC_REG_DEF) => "GET_REVOC_REG_DEF",
                Some(GET_REVOC_REG) => "GET_REVOC_REG",
                Some(GET_REVOC_REG_DELTA) => "GET_REVOC_REG_DELTA",
                Some(POOL_RESTART) => "POOL_RESTART",
                Some(GET_VALIDATOR_INFO) => "GET_VALIDATOR_INFO",
                Some(AUTH_RULE) => "AUTH_RULE",
                Some(GET_AUTH_RULE) => "GET_AUTH_RULE",
                Some(AUTH_RULES) => "AUTH_RULES",
                Some(val) => val,
                _ => "-",
            }
            .to_string(),
        )
    }
}
