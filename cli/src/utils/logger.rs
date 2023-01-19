pub struct IndyCliLogger;

impl IndyCliLogger {
    pub fn init(path: &str) -> Result<(), String> {
        log4rs::init_file(path, Default::default())
            .map_err(|err| format!("Cannot init Indy CLI logger: {}", err.to_string()))?;
        Ok(())
    }
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! secret {
    ($val:expr) => {{
        $val
    }};
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! secret {
    ($val:expr) => {{
        "_"
    }};
}
