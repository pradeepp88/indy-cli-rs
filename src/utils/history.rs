use std::fs::DirBuilder;
use linefeed::{Terminal, Interface};

use crate::utils::{
    environment::EnvironmentUtils,
    file::{read_lines_from_file},
};

const HISTORY_SIZE: usize = 100;
const SECRET_DATA: [&str; 2] = [" seed=", " key="];

pub fn load<T>(reader: &mut Interface<T>) -> Result<(), String>
    where
        T: Terminal,
{
    reader.set_history_size(HISTORY_SIZE);

    let path = EnvironmentUtils::history_file_path();

    for line in read_lines_from_file(path)? {
        if let Ok(line) = line {
            reader.add_history(line)
        }
    }
    Ok(())
}

pub fn add<T>(line: &str, reader: &Interface<T>) -> Result<(), String>
    where
        T: Terminal,
{
    let has_secrets = SECRET_DATA
        .iter()
        .any(|secret_word| line.contains(secret_word));

    if !has_secrets {
        reader.add_history(line.to_string());
    }
    Ok(())
}

pub fn persist<T>(reader: &Interface<T>) -> Result<(), String>
    where
        T: Terminal,
{
    let path = EnvironmentUtils::history_file_path();
    if let Some(parent_path) = path.parent() {
        if !parent_path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(parent_path)
                .map_err(|err| format!("Can't create the file: {}", err))?;
        }
    }

    reader.save_history(path)
        .map_err(|err| format!("Can't store CLI history into the file: {}", err))?;
    Ok(())
}
