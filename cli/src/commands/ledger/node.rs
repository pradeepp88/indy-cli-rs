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

use indy_vdr::ledger::requests::node::{NodeOperationData, Services};
use serde_json::Value as JsonValue;

use super::common::{handle_transaction_response, print_transaction_response};

pub mod node_command {
    use super::*;

    command!(CommandMetadata::build("node", "Send Node transaction to the Ledger.")
                .add_required_param("target", "Node identifier")
                .add_required_param("alias", "Node alias (can't be changed in case of update)")
                .add_optional_param("node_ip", "Node Ip. Note that it is mandatory for adding node case")
                .add_optional_param("node_port", "Node port. Note that it is mandatory for adding node case")
                .add_optional_param("client_ip", "Client Ip. Note that it is mandatory for adding node case")
                .add_optional_param("client_port","Client port. Note that it is mandatory for adding node case")
                .add_optional_param("blskey",  "Node BLS key")
                .add_optional_param("blskey_pop",  "Node BLS key proof of possession. Note that it is mandatory if blskey specified")
                .add_optional_param("services", "Node type. One of: VALIDATOR, OBSERVER or empty in case of blacklisting node")
                .add_optional_param("sign","Sign the request (True by default)")
                .add_optional_param("send","Send the request to the Ledger (True by default). If false then created request will be printed and stored into CLI context.")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y node_ip=127.0.0.1 node_port=9710 client_ip=127.0.0.1 client_port=9711 alias=Node5 services=VALIDATOR blskey=2zN3bHM1m4rLz54MJHYSwvqzPchYp8jkHswveCLAEJVcX6Mm1wHQD1SkPYMzUDTZvWvhuE6VNAkK3KxVeEmsanSmvjVkReDeBEMxeDaayjcZjFGPydyey1qxBHmTvAnBKoPydvuTAqx5f7YNNRAdeLmUi99gERUU7TD8KfAa6MpQ9bw blskey_pop=RPLagxaR5xdimFzwmzYnz4ZhWtYQEj8iR5ZU53T2gitPCyCHQneUn2Huc4oeLd2B2HzkGnjAff4hWTJT6C7qHYB1Mv2wU5iHHGFWkhnTX9WsEAbunJCV2qcaXScKj4tTfvdDKfLiVuU2av6hbsMztirRze7LvYBkRHV3tGwyCptsrP")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y node_ip=127.0.0.1 node_port=9710 client_ip=127.0.0.1 client_port=9711 alias=Node5 services=VALIDATOR")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y alias=Node5 services=VALIDATOR")
                .add_example("ledger node target=A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y alias=Node5 services=")
                .finalize()
    );

    fn execute(ctx: &CommandContext, params: &CommandParams) -> Result<(), ()> {
        trace!("execute >> ctx {:?} params {:?}", ctx, params);

        let wallet = ctx.ensure_opened_wallet()?;
        let submitter_did = ctx.ensure_active_did()?;
        let pool = ctx.get_connected_pool();

        let target_did = ParamParser::get_did_param("target", params)?;
        let alias = ParamParser::get_str_param("alias", params)?;
        let node_ip = ParamParser::get_opt_str_param("node_ip", params)?;
        let node_port = ParamParser::get_opt_number_param::<i32>("node_port", params)?;
        let client_ip = ParamParser::get_opt_str_param("client_ip", params)?;
        let client_port = ParamParser::get_opt_number_param::<i32>("client_port", params)?;
        let blskey = ParamParser::get_opt_str_param("blskey", params)?;
        let blskey_pop = ParamParser::get_opt_str_param("blskey_pop", params)?;
        let services = ParamParser::get_opt_str_array_param("services", params)?;

        let services = match services {
            Some(services) => Some(
                services
                    .into_iter()
                    .map(|service| match service {
                        "VALIDATOR" => Ok(Services::VALIDATOR),
                        "OBSERVER" => Ok(Services::OBSERVER),
                        service => {
                            println_err!("Unsupported service \"{}\"!", service);
                            Err(())
                        }
                    })
                    .collect::<Result<Vec<Services>, ()>>()?,
            ),
            None => None,
        };

        let node_data = NodeOperationData {
            node_ip: node_ip.map(String::from),
            node_port,
            client_ip: client_ip.map(String::from),
            client_port,
            alias: alias.to_string(),
            services,
            blskey: blskey.map(String::from),
            blskey_pop: blskey_pop.map(String::from),
        };

        let mut request =
            Ledger::build_node_request(pool.as_deref(), &submitter_did, &target_did, node_data)
                .map_err(|err| println_err!("{}", err.message(None)))?;

        let (_, response): (String, Response<JsonValue>) =
            send_write_request!(ctx, params, &mut request, &wallet, &submitter_did);

        handle_transaction_response(response).map(|result| {
            print_transaction_response(
                result,
                "NodeConfig request has been sent to Ledger.",
                Some("data"),
                &[
                    ("alias", "Alias"),
                    ("node_ip", "Node Ip"),
                    ("node_port", "Node Port"),
                    ("client_ip", "Client Ip"),
                    ("client_port", "Client Port"),
                    ("services", "Services"),
                    ("blskey", "Blskey"),
                    ("blskey_pop", "Blskey Proof of Possession"),
                ],
                true,
            )
        })?;
        trace!("execute <<");
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        commands::{
            did::tests::use_did, setup_with_wallet_and_pool, tear_down_with_wallet_and_pool,
        },
        ledger::tests::{create_new_did, send_nym, use_trustee},
    };

    mod node {
        use super::*;

        #[test]
        #[ignore] //TODO: FIXME currently unstable pool behaviour after new non-existing node was added
        pub fn node_works() {
            let ctx = setup_with_wallet_and_pool();
            use_trustee(&ctx);
            let (_did, my_verkey) = create_new_did(&ctx);
            send_nym(&ctx, &_did, &my_verkey, Some("STEWARD"));
            use_did(&ctx, &_did);
            {
                let cmd = node_command::new();
                let mut params = CommandParams::new();
                params.insert(
                    "target",
                    "A5iWQVT3k8Zo9nXj4otmeqaUziPQPCiDqcydXkAJBk1Y".to_string(),
                );
                params.insert("node_ip", "127.0.0.1".to_string());
                params.insert("node_port", "9710".to_string());
                params.insert("client_ip", "127.0.0.2".to_string());
                params.insert("client_port", "9711".to_string());
                params.insert("alias", "Node5".to_string());
                params.insert("blskey", "2zN3bHM1m4rLz54MJHYSwvqzPchYp8jkHswveCLAEJVcX6Mm1wHQD1SkPYMzUDTZvWvhuE6VNAkK3KxVeEmsanSmvjVkReDeBEMxeDaayjcZjFGPydyey1qxBHmTvAnBKoPydvuTAqx5f7YNNRAdeLmUi99gERUU7TD8KfAa6MpQ9bw".to_string());
                params.insert("blskey_pop", "RPLagxaR5xdimFzwmzYnz4ZhWtYQEj8iR5ZU53T2gitPCyCHQneUn2Huc4oeLd2B2HzkGnjAff4hWTJT6C7qHYB1Mv2wU5iHHGFWkhnTX9WsEAbunJCV2qcaXScKj4tTfvdDKfLiVuU2av6hbsMztirRze7LvYBkRHV3tGwyCptsrP".to_string());
                params.insert("services", "VALIDATOR".to_string());
                cmd.execute(&ctx, &params).unwrap();
            }
            tear_down_with_wallet_and_pool(&ctx);
        }
    }
}
