/*
    Copyright 2023 DSR Corporation, Denver, Colorado.
    https://www.dsr-corporation.com
    SPDX-License-Identifier: Apache-2.0
*/
use crate::command_executor::CommandParams;

use indy_utils::{did::DidValue, Qualifiable};
use std::{fmt::Display, str::FromStr};

pub struct ParamParser;

impl ParamParser {
    pub fn get_str_param<'a>(name: &'a str, params: &'a CommandParams) -> Result<&'a str, ()> {
        match params.get(name) {
            Some(v) if v == "" => {
                println_err!("Required \"{}\" parameter is empty.", name);
                Err(())
            }
            Some(v) => Ok(v.as_str()),
            None => {
                println_err!("No required \"{}\" parameter present.", name);
                Err(())
            }
        }
    }

    pub fn get_opt_str_param<'a>(
        key: &'a str,
        params: &'a CommandParams,
    ) -> Result<Option<&'a str>, ()> {
        match params.get(key) {
            Some(v) if v == "" => {
                println_err!("Optional parameter \"{}\" is empty.", key);
                Err(())
            }
            Some(v) => Ok(Some(v.as_str())),
            None => Ok(None),
        }
    }

    pub fn get_opt_empty_str_param<'a>(
        key: &'a str,
        params: &'a CommandParams,
    ) -> Result<Option<&'a str>, ()> {
        Ok(params.get(key).map(|v| v.as_str()))
    }

    pub fn _get_int_param<T>(name: &str, params: &CommandParams) -> Result<T, ()>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        match params.get(name) {
            Some(v) => Ok(v.parse::<T>().map_err(|err| {
                println_err!("Can't parse integer parameter \"{}\": err {}", name, err)
            })?),
            None => {
                println_err!("No required \"{}\" parameter present", name);
                Err(())
            }
        }
    }

    pub fn get_number_param<T>(key: &str, params: &CommandParams) -> Result<T, ()>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        match params.get(key) {
            Some(value) => value.parse::<T>().map_err(|err| {
                println_err!(
                    "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                    key,
                    value,
                    err
                )
            }),
            None => {
                println_err!("No required \"{}\" parameter present", key);
                Err(())
            }
        }
    }

    pub fn get_opt_number_param<T>(key: &str, params: &CommandParams) -> Result<Option<T>, ()>
    where
        T: FromStr,
        <T as FromStr>::Err: Display,
    {
        let res = match params.get(key) {
            Some(value) => Some(value.parse::<T>().map_err(|err| {
                println_err!(
                    "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                    key,
                    value,
                    err
                )
            })?),
            None => None,
        };
        Ok(res)
    }

    pub fn get_bool_param(key: &str, params: &CommandParams) -> Result<bool, ()> {
        match params.get(key) {
            Some(value) => Ok(value.parse::<bool>().map_err(|err| {
                println_err!("Can't parse bool parameter \"{}\": err {}", key, err)
            })?),
            None => {
                println_err!("No required \"{}\" parameter present", key);
                Err(())
            }
        }
    }

    pub fn get_opt_bool_param(key: &str, params: &CommandParams) -> Result<Option<bool>, ()> {
        match params.get(key) {
            Some(value) => Ok(Some(value.parse::<bool>().map_err(|err| {
                println_err!("Can't parse bool parameter \"{}\": err {}", key, err)
            })?)),
            None => Ok(None),
        }
    }

    pub fn get_str_array_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<Vec<&'a str>, ()> {
        match params.get(name) {
            None => {
                println_err!("No required \"{}\" parameter present", name);
                Err(())
            }
            Some(v) if v.is_empty() => {
                println_err!("No required \"{}\" parameter present", name);
                Err(())
            }
            Some(v) => Ok(v.split(',').collect::<Vec<&'a str>>()),
        }
    }

    pub fn get_opt_str_array_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<Option<Vec<&'a str>>, ()> {
        match params.get(name) {
            Some(v) => {
                if v.is_empty() {
                    Ok(Some(Vec::<&'a str>::new()))
                } else {
                    Ok(Some(v.split(',').collect::<Vec<&'a str>>()))
                }
            }
            None => Ok(None),
        }
    }

    pub fn get_object_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<serde_json::Value, ()> {
        match params.get(name) {
            Some(v) => Ok(serde_json::from_str(v).map_err(|err| {
                println_err!("Can't parse object parameter \"{}\": err {}", name, err)
            })?),
            None => {
                println_err!("No required \"{}\" parameter present", name);
                Err(())
            }
        }
    }

    pub fn get_opt_object_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<Option<serde_json::Value>, ()> {
        match params.get(name) {
            Some(_) => Ok(Some(ParamParser::get_object_param(name, params)?)),
            None => Ok(None),
        }
    }

    pub fn get_number_tuple_array_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<Vec<u64>, ()> {
        match params.get(name) {
            Some(v) if !v.is_empty() => {
                let tuples: Vec<&str> = v.split(",").collect();
                if tuples.is_empty() {
                    println_err!("Parameter \"{}\" has invalid format", name);
                    Err(())
                } else {
                    let mut result: Vec<u64> = Vec::new();
                    for item in tuples {
                        println!("{:?}", item);
                        result.push(item.parse::<u64>().map_err(|err| {
                            println_err!(
                                "Can't parse number parameter \"{}\": value: \"{}\", err \"{}\"",
                                name,
                                item,
                                err
                            )
                        })?);
                    }
                    Ok(result)
                }
            }
            _ => {
                println_err!("No required \"{}\" parameter present", name);
                Err(())
            }
        }
    }

    pub fn get_did_param<'a>(name: &'a str, params: &'a CommandParams) -> Result<DidValue, ()> {
        let did_str = ParamParser::get_str_param(name, params)?;
        DidValue::from_str(did_str).map_err(|_| println_err!("Invalid DID {} provided", did_str))
    }

    pub fn get_opt_did_param<'a>(
        name: &'a str,
        params: &'a CommandParams,
    ) -> Result<Option<DidValue>, ()> {
        let did_str = ParamParser::get_opt_str_param(name, params)?;
        match did_str {
            Some(did_str) => {
                Ok(Some(DidValue::from_str(did_str).map_err(|_| {
                    println_err!("Invalid DID {} provided", did_str)
                })?))
            }
            None => Ok(None),
        }
    }
}
