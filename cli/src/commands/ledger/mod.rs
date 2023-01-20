/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{CommandGroup, CommandGroupMetadata};

#[macro_use]
pub mod common;
pub mod attrib;
pub mod auth_rule;
pub mod constants;
pub mod cred_def;
pub mod custom;
pub mod endorser;
pub mod frozen_ledger;
pub mod node;
pub mod nym;
pub mod pool_config;
pub mod pool_restart;
pub mod pool_upgrade;
pub mod schema;
pub mod sign_multi;
pub mod transaction;
pub mod transaction_author_agreement;
pub mod validator_info;

pub use self::{
    attrib::*, auth_rule::*, common::*, cred_def::*, custom::*, endorser::*, frozen_ledger::*,
    node::*, nym::*, pool_config::*, pool_restart::*, pool_upgrade::*, schema::*, sign_multi::*,
    transaction::*, transaction_author_agreement::*, validator_info::*,
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "ledger",
        "Ledger management commands"
    ));
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        command_executor::{CommandContext, CommandParams},
        commands::did::tests::{new_did, use_did, DID_TRUSTEE, SEED_TRUSTEE},
        tools::did::Did,
    };

    #[derive(Deserialize, Debug)]
    pub struct ReplyResult<T> {
        pub data: T,
    }

    pub const TRANSACTION: &str = r#"{"reqId":1,"identifier":"V4SGRU86Z58d6TV7PBUe6f","operation":{"type":"105","dest":"V4SGRU86Z58d6TV7PBUe6f"},"protocolVersion":2}"#;

    pub fn create_new_did(ctx: &CommandContext) -> (String, String) {
        let wallet = ctx.get_opened_wallet().unwrap();
        Did::create(&wallet, None, None, None, None).unwrap()
    }

    pub fn use_trustee(ctx: &CommandContext) {
        new_did(&ctx, SEED_TRUSTEE);
        use_did(&ctx, DID_TRUSTEE);
    }

    pub fn use_new_identity(ctx: &CommandContext) -> (String, String) {
        use_trustee(ctx);
        let (did, verkey) = create_new_did(ctx);
        send_nym(ctx, &did, &verkey, None);
        use_did(&ctx, &did);
        (did, verkey)
    }

    pub fn use_new_endorser(ctx: &CommandContext) -> (String, String) {
        use_trustee(ctx);
        let (did, verkey) = create_new_did(ctx);
        send_nym(ctx, &did, &verkey, Some("ENDORSER"));
        use_did(&ctx, &did);
        (did, verkey)
    }

    pub fn send_nym(ctx: &CommandContext, did: &str, verkey: &str, role: Option<&str>) {
        let cmd = nym_command::new();
        let mut params = CommandParams::new();
        params.insert("did", did.to_string());
        params.insert("verkey", verkey.to_string());
        if let Some(role) = role {
            params.insert("role", role.to_string());
        }
        cmd.execute(&ctx, &params).unwrap();
    }
}
