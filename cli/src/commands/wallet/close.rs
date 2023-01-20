/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    tools::wallet::Wallet,
};

pub mod close_command {
    use super::*;

    command!(CommandMetadata::build("close", "Close opened wallet.").finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        if let Some(wallet) = ctx.take_opened_wallet()? {
            close_wallet(ctx, wallet)?;
        } else {
            println_err!("There is no opened wallet now");
            return Err(());
        }

        trace!("CloseCommand::execute <<");
        Ok(())
    }
}

pub fn close_wallet(ctx: &CommandContext, wallet: Wallet) -> Result<(), ()> {
    let name = wallet.name.clone();
    wallet
        .close()
        .map(|_| {
            ctx.reset_opened_wallet();
            ctx.reset_active_did();
            println_succ!("Wallet \"{}\" has been closed", name);
        })
        .map_err(|err| println_err!("{}", err.message(Some(&name))))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::commands::{setup_with_wallet, tear_down};

    mod close {
        use super::*;
        use crate::{
            commands::setup,
            wallet::tests::{create_and_open_wallet, create_wallet, delete_wallet},
        };

        #[test]
        pub fn close_works() {
            let ctx = setup_with_wallet();
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            ctx.ensure_opened_wallet().unwrap_err();
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn close_works_for_not_opened() {
            let ctx = setup();
            create_wallet(&ctx);
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }

        #[test]
        pub fn close_works_for_twice() {
            let ctx = setup();
            create_and_open_wallet(&ctx);
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap();
            }
            {
                let cmd = close_command::new();
                let params = CommandParams::new();
                cmd.execute(&ctx, &params).unwrap_err();
            }
            delete_wallet(&ctx);
            tear_down();
        }
    }
}
