/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
pub mod helpers;
pub mod response;

use crate::{
    error::{CliError, CliResult},
    tools::did::Did,
    utils::futures::block_on,
};

use crate::tools::{pool::Pool, wallet::Wallet};
use indy_utils::did::DidValue;
use indy_vdr::{
    ledger::{
        identifiers::{CredentialDefinitionId, SchemaId},
        requests::{
            auth_rule::{AddAuthRuleData, AuthRuleData, AuthRules, Constraint, EditAuthRuleData},
            author_agreement::{AcceptanceMechanisms, GetTxnAuthorAgreementData},
            cred_def::CredentialDefinition,
            node::NodeOperationData,
            pool::Schedule,
            schema::Schema,
        },
        RequestBuilder,
    },
    pool::{
        helpers::{perform_ledger_action, perform_ledger_request},
        NodeReplies, Pool as PoolImpl, PreparedRequest, ProtocolVersion, RequestResult,
    },
};
use serde_json::Value as JsonValue;

pub use self::{
    helpers::LedgerHelpers,
    response::{parse_transaction_response, Response, ResponseType},
};

pub struct Ledger {}

impl Ledger {
    pub fn sign_and_submit_request(
        pool: &Pool,
        store: &Wallet,
        submitter_did: &DidValue,
        request: &mut PreparedRequest,
    ) -> CliResult<String> {
        block_on(async move {
            let signature = Self::_sign(request, store, submitter_did).await?;
            request.set_signature(&signature)?;
            Self::_submit_request(request, pool).await
        })
    }

    pub fn submit_request(pool: &Pool, request: &PreparedRequest) -> CliResult<String> {
        block_on(async { Self::_submit_request(request, pool).await })
    }

    pub fn submit_action(
        pool: &Pool,
        request: &PreparedRequest,
        nodes: Option<&str>,
        timeout: Option<i64>,
    ) -> CliResult<NodeReplies<String>> {
        let nodes: Option<Vec<String>> = match nodes {
            Some(nodes) => Some(serde_json::from_str::<Vec<String>>(nodes)?),
            None => None,
        };

        block_on(async {
            let (request_result, _) = perform_ledger_action(
                &pool.pool,
                request.req_id.to_string(),
                request.req_json.to_string(),
                nodes,
                timeout,
            )
            .await?;
            match request_result {
                RequestResult::Reply(message) => Ok(message),
                RequestResult::Failed(error) => Err(error.into()),
            }
        })
    }

    pub fn sign_request(
        store: &Wallet,
        did: &DidValue,
        request: &mut PreparedRequest,
    ) -> CliResult<()> {
        block_on(async move {
            let signature = Self::_sign(request, store, did).await?;
            request.set_signature(&signature).map_err(CliError::from)
        })
    }

    pub fn multi_sign_request(
        store: &Wallet,
        did: &DidValue,
        request: &mut PreparedRequest,
    ) -> CliResult<()> {
        block_on(async move {
            let signature = Self::_sign(request, store, did).await?;
            request
                .set_multi_signature(did, &signature)
                .map_err(CliError::from)
        })
    }

    pub fn build_nym_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        target_did: &DidValue,
        verkey: Option<&str>,
        data: Option<&str>,
        role: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_nym_request(
                submitter_did,
                target_did,
                verkey.map(String::from),
                data.map(String::from),
                role.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_get_nym_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        target_did: &DidValue,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_nym_request(submitter_did, target_did)
            .map_err(CliError::from)
    }

    pub fn build_attrib_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        target_did: &DidValue,
        hash: Option<&str>,
        raw: Option<&JsonValue>,
        enc: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_attrib_request(
                submitter_did,
                target_did,
                hash.map(String::from),
                raw,
                enc.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_get_attrib_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        target_did: &DidValue,
        raw: Option<&str>,
        hash: Option<&str>,
        enc: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_attrib_request(
                submitter_did,
                target_did,
                raw.map(String::from),
                hash.map(String::from),
                enc.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_schema_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        schema: Schema,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_schema_request(submitter_did, schema)
            .map_err(CliError::from)
    }

    pub fn build_get_schema_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        id: &SchemaId,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_schema_request(submitter_did, id)
            .map_err(CliError::from)
    }

    pub fn build_cred_def_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        cred_def: CredentialDefinition,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_cred_def_request(submitter_did, cred_def)
            .map_err(CliError::from)
    }

    pub fn build_get_validator_info_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_validator_info_request(submitter_did)
            .map_err(CliError::from)
    }

    pub fn build_get_cred_def_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        id: &CredentialDefinitionId,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_cred_def_request(submitter_did, id)
            .map_err(CliError::from)
    }

    pub fn build_node_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        target_did: &DidValue,
        node_data: NodeOperationData,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_node_request(submitter_did, target_did, node_data)
            .map_err(CliError::from)
    }

    pub fn indy_build_pool_config_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        writes: bool,
        force: bool,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_pool_config(submitter_did, writes, force)
            .map_err(CliError::from)
    }

    pub fn indy_build_pool_restart_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        action: &str,
        datetime: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_pool_restart(submitter_did, action, datetime)
            .map_err(CliError::from)
    }

    pub fn indy_build_pool_upgrade_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        name: &str,
        version: &str,
        action: &str,
        sha256: &str,
        timeout: Option<u32>,
        schedule: Option<&str>,
        justification: Option<&str>,
        reinstall: bool,
        force: bool,
        package: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        let schedule: Option<Schedule> = match schedule {
            Some(schedule) => Some(serde_json::from_str::<Schedule>(schedule).map_err(|_| {
                CliError::InvalidInput(format!("Invalid value provided for `schedule` parameter"))
            })?),
            None => None,
        };

        Self::_request_builder(pool)
            .build_pool_upgrade(
                submitter_did,
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
            .map_err(CliError::from)
    }

    pub fn build_auth_rule_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        txn_type: &str,
        action: &str,
        field: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        constraint: &str,
    ) -> CliResult<PreparedRequest> {
        let constraint: Constraint = serde_json::from_str(constraint)?;

        let auth_rule = match action {
            "ADD" => AuthRuleData::Add(AddAuthRuleData {
                auth_type: txn_type.to_string(),
                field: field.to_string(),
                new_value: new_value.map(String::from),
                constraint,
            }),
            "EDIT" => AuthRuleData::Edit(EditAuthRuleData {
                auth_type: txn_type.to_string(),
                field: field.to_string(),
                old_value: old_value.map(String::from),
                new_value: new_value.map(String::from),
                constraint,
            }),
            _ => {
                return Err(CliError::InvalidInput(format!(
                    "Unexpected auth rule action {}",
                    action
                )));
            }
        };
        let rules = vec![auth_rule];
        Self::_request_builder(pool)
            .build_auth_rules_request(submitter_did, rules)
            .map_err(CliError::from)
    }

    pub fn build_auth_rules_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        rules: &str,
    ) -> CliResult<PreparedRequest> {
        let rules: AuthRules = serde_json::from_str(rules)?;

        Self::_request_builder(pool)
            .build_auth_rules_request(submitter_did, rules)
            .map_err(CliError::from)
    }

    pub fn build_get_auth_rule_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        auth_type: Option<&str>,
        auth_action: Option<&str>,
        field: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_auth_rule_request(
                submitter_did,
                auth_type.map(String::from),
                auth_action.map(String::from),
                field.map(String::from),
                old_value.map(String::from),
                new_value.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_txn_author_agreement_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        text: Option<&str>,
        version: &str,
        ratification_ts: Option<u64>,
        retirement_ts: Option<u64>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_txn_author_agreement_request(
                submitter_did,
                text.map(String::from),
                version.to_string(),
                ratification_ts,
                retirement_ts,
            )
            .map_err(CliError::from)
    }

    pub fn build_disable_all_txn_author_agreements_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_disable_all_txn_author_agreements_request(submitter_did)
            .map_err(CliError::from)
    }

    pub fn build_acceptance_mechanisms_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        aml: &str,
        version: &str,
        aml_context: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        let aml: AcceptanceMechanisms = serde_json::from_str(aml)?;

        Self::_request_builder(pool)
            .build_acceptance_mechanisms_request(
                submitter_did,
                aml,
                version.to_string(),
                aml_context.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_get_acceptance_mechanisms_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        timestamp: Option<u64>,
        version: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_acceptance_mechanisms_request(
                submitter_did,
                timestamp,
                version.map(String::from),
            )
            .map_err(CliError::from)
    }

    pub fn build_get_txn_author_agreement_request(
        pool: Option<&Pool>,
        submitter_did: Option<&DidValue>,
        data: Option<&str>,
    ) -> CliResult<PreparedRequest> {
        let data: Option<GetTxnAuthorAgreementData> = match data {
            Some(data) => Some(serde_json::from_str::<GetTxnAuthorAgreementData>(data)?),
            None => None,
        };

        Self::_request_builder(pool)
            .build_get_txn_author_agreement_request(submitter_did, data.as_ref())
            .map_err(CliError::from)
    }

    pub fn append_txn_author_agreement_acceptance_to_request(
        pool: Option<&Pool>,
        request: &mut PreparedRequest,
        text: Option<&str>,
        version: Option<&str>,
        hash: Option<&str>,
        acc_mech_type: &str,
        time_of_acceptance: u64,
    ) -> CliResult<()> {
        let data = Self::_request_builder(pool).prepare_txn_author_agreement_acceptance_data(
            text,
            version,
            hash,
            acc_mech_type,
            time_of_acceptance,
        )?;

        request
            .set_txn_author_agreement_acceptance(&data)
            .map_err(CliError::from)
    }

    pub fn append_request_endorser(
        request: &mut PreparedRequest,
        endorser_did: &DidValue,
    ) -> CliResult<()> {
        request.set_endorser(endorser_did).map_err(CliError::from)
    }

    pub fn build_ledgers_freeze_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
        ledgers_ids: Vec<u64>,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_ledger_freeze_request(submitter_did, &ledgers_ids)
            .map_err(CliError::from)
    }

    pub fn build_get_frozen_ledgers_request(
        pool: Option<&Pool>,
        submitter_did: &DidValue,
    ) -> CliResult<PreparedRequest> {
        Self::_request_builder(pool)
            .build_get_frozen_ledgers_request(submitter_did)
            .map_err(CliError::from)
    }

    fn _request_builder(pool: Option<&Pool>) -> RequestBuilder {
        pool.map(|pool| pool.pool.get_request_builder())
            .unwrap_or_else(|| RequestBuilder::new(ProtocolVersion::Node1_4))
    }

    async fn _submit_request(request: &PreparedRequest, pool: &Pool) -> CliResult<String> {
        let (request_result, _) = perform_ledger_request(&pool.pool, request).await?;
        match request_result {
            RequestResult::Reply(message) => Ok(message),
            RequestResult::Failed(error) => Err(error.into()),
        }
    }

    async fn _sign(
        request: &mut PreparedRequest,
        store: &Wallet,
        submitter_did: &DidValue,
    ) -> CliResult<Vec<u8>> {
        let sig_bytes = request.get_signature_input()?;
        Did::sign(store, &submitter_did.to_string(), sig_bytes.as_bytes()).await
    }
}
