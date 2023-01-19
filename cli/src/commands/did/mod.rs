/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::{CommandGroup, CommandGroupMetadata};

pub mod import;
pub mod list;
pub mod new;
pub mod qualify;
pub mod rotate_key;
pub mod set_metadata;
pub mod use_did;

pub use self::{
    import::*, list::*, new::*, qualify::*, rotate_key::*, set_metadata::*, use_did::*,
};

pub mod group {
    use super::*;

    command_group!(CommandGroupMetadata::new(
        "did",
        "Identity management commands"
    ));
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        command_executor::{CommandContext, CommandParams},
        tools::did::{Did, DidInfo},
    };
    use indy_utils::did::DidValue;

    pub const SEED_TRUSTEE: &'static str = "000000000000000000000000Trustee1";
    pub const DID_TRUSTEE: &'static str = "V4SGRU86Z58d6TV7PBUe6f";
    pub const VERKEY_TRUSTEE: &'static str = "GJ1SzoWzavQYfNL9XkaJdrQejfztN4XqdsiV4ct3LXKL";

    pub const SEED_MY1: &'static str = "00000000000000000000000000000My1";
    pub const DID_MY1: &'static str = "VsKV7grR1BUE29mG2Fm2kX";
    pub const VERKEY_MY1: &'static str = "GjZWsBLgZCR18aL468JAT7w9CZRiBnpxUPPgyQxh4voa";

    pub const SEED_MY3: &'static str = "00000000000000000000000000000My3";
    pub const DID_MY3: &'static str = "5Uu7YveFSGcT3dSzjpvPab";
    pub const VERKEY_MY3: &'static str = "3SeuRm3uYuQDYmHeuMLu1xNHozNTtzS3kbZRFMMCWrX4";

    pub fn get_did_info(ctx: &CommandContext, did: &str) -> DidInfo {
        let wallet = ctx.ensure_opened_wallet().unwrap();
        let did = DidValue(did.to_string());
        Did::get(&wallet, &did).unwrap()
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
