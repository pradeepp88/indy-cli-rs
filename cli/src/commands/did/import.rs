/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::{
    command_executor::{Command, CommandContext, CommandMetadata, CommandParams},
    params_parser::ParamParser,
    tools::did::Did,
};

pub mod import_command {
    use super::*;
    use crate::utils::file::read_file;

    #[derive(Debug, Deserialize)]
    struct DidImportConfig {
        version: usize,
        dids: Vec<DidImportInfo>,
    }

    #[derive(Debug, Deserialize)]
    struct DidImportInfo {
        did: Option<String>,
        seed: String,
    }

    command!(CommandMetadata::build(
        "import",
        "Import DIDs entities from file to the current wallet.
        File format:
        {
            \"version\": 1,
            \"dids\": [{
                \"did\": \"did\",
                \"seed\": \"UTF-8, base64 or hex string\"
            }]
        }"
    )
    .add_main_param("file", "Path to file with DIDs")
    .finalize());

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let store = ctx.ensure_opened_wallet()?;

        let path = ParamParser::get_str_param("file", params)?;

        let data = read_file(path)
            .map_err(|_| println_err!("Unable to read DID import config from the provided file"))?;

        let config: DidImportConfig = serde_json::from_str(&data)
            .map_err(|_| println_err!("Unable to read DID import config from the provided file"))?;

        if config.version != 1 {
            println_err!("Unsupported DID import config version");
            return Err(());
        }

        for did in config.dids {
            let (did, vk) = Did::create(
                &store,
                did.did.as_ref().map(String::as_str),
                Some(&did.seed),
                None,
                None,
            )
            .map_err(|err| println_err!("{}", err.message(None)))?;

            let vk = Did::abbreviate_verkey(&did, &vk)
                .map_err(|err| println_err!("{}", err.message(None)))?;

            println_succ!("Did \"{}\" has been created with \"{}\" verkey", did, vk)
        }

        println_succ!("DIDs import finished");

        trace!("execute << ");
        Ok(())
    }
}
