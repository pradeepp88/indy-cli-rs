/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
};

pub mod set_protocol_version_command {
    use super::*;

    command!(CommandMetadata::build(
        "set-protocol-version",
        "Set protocol version that will be used for ledger requests. One of: 1, 2. \
                 Unless command is called the default protocol version 2 is used."
    )
    .add_main_param("protocol-version", "Protocol version to use")
    .add_example("pool set-protocol-version 2")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let protocol_version = ParamParser::get_number_param::<usize>("protocol-version", params)?;

        ctx.set_pool_protocol_version(protocol_version);
        println_succ!("Protocol Version has been set: \"{}\".", protocol_version);

        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{setup, tear_down},
        pool::constants::DEFAULT_POOL_PROTOCOL_VERSION,
    };

    mod set_protocol_version {

        use super::*;
        use crate::pool::tests::create_pool;

        #[test]
        pub fn set_protocol_version_works() {
            let ctx = setup();
            create_pool(&ctx);
            {
                let cmd = set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "protocol-version",
                    DEFAULT_POOL_PROTOCOL_VERSION.to_string(),
                );
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = set_protocol_version_command::new();
                let mut params = CommandParams::new();
                params.insert("protocol-version", "invalid".to_string());
                cmd.execute(&ctx, &params).unwrap_err();
            }

            tear_down();
        }
    }
}
