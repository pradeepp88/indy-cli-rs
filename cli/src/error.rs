use aries_askar::{Error as AskarError, ErrorKind as AskarErrorKind};
use indy_utils::{ConversionError, ValidationError};
use indy_vdr::common::error::{VdrError, VdrErrorKind};
use serde_json::Error as SerdeError;
use std::io::Error as FileSystemError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("`{0}`")]
    Duplicate(String),
    #[error("`{0}`")]
    NotFound(String),
    #[error("`{0}`")]
    InvalidEntityState(String),
    #[error("`{0}`")]
    Unimplemented(String),
    #[error("Invalid input parameter provided `{0}`")]
    InvalidInput(String),
    #[error("Aries Askar error occurred `{0}`")]
    AskarError(AskarError),
    #[error("Aries Askar error occurred `{0}`")]
    VdrError(VdrError),
    #[error("File System error occurred `{0}`")]
    FileSystemError(FileSystemError),
}

impl From<ValidationError> for CliError {
    fn from(err: ValidationError) -> Self {
        CliError::InvalidInput(err.to_string())
    }
}

impl From<AskarError> for CliError {
    fn from(err: AskarError) -> Self {
        CliError::AskarError(err)
    }
}

impl From<VdrError> for CliError {
    fn from(err: VdrError) -> Self {
        CliError::VdrError(err)
    }
}

impl From<FileSystemError> for CliError {
    fn from(err: FileSystemError) -> Self {
        CliError::FileSystemError(err)
    }
}

impl From<SerdeError> for CliError {
    fn from(err: SerdeError) -> Self {
        CliError::InvalidInput(err.to_string())
    }
}

impl From<ConversionError> for CliError {
    fn from(err: ConversionError) -> Self {
        CliError::InvalidInput(err.to_string())
    }
}

pub type CliResult<T> = Result<T, CliError>;

impl CliError {
    pub fn message(&self, extra: Option<&str>) -> String {
        match self {
            CliError::InvalidInput(message)
            | CliError::InvalidEntityState(message)
            | CliError::NotFound(message)
            | CliError::Duplicate(message)
            | CliError::Unimplemented(message) => message.to_string(),
            CliError::VdrError(vdr_error) => match vdr_error.kind() {
                VdrErrorKind::Config => "Pool configuration is invalid.".to_string(),
                VdrErrorKind::Connection => format!(
                    "Pool \"{}\" has not been connected.",
                    extra.unwrap_or_default()
                ),
                VdrErrorKind::FileSystem(_) => format!(
                    "Pool  \"{}\" genesis transactions file does not exist.",
                    extra.unwrap_or_default()
                ),
                VdrErrorKind::Input => vdr_error.to_string(),
                VdrErrorKind::Resource => format!("Unable to send request."),
                VdrErrorKind::Unavailable => format!("Pool unavailable."),
                VdrErrorKind::Unexpected => format!(
                    "Unexpected pool error occurred: {:?}",
                    vdr_error.to_string()
                ),
                VdrErrorKind::Incompatible => format!(
                    "Pool \"{}\" is not compatible with protocol version.",
                    extra.unwrap_or_default()
                ),
                VdrErrorKind::PoolNoConsensus => {
                    format!("Unable to send request because there is not consensus from verifiers.")
                }
                VdrErrorKind::PoolTimeout => format!("Transaction response has not been received"),
                VdrErrorKind::PoolRequestFailed(reason) => {
                    let reason = serde_json::from_str::<serde_json::Value>(&reason)
                        .ok()
                        .and_then(|value| value["reason"].as_str().map(String::from))
                        .unwrap_or(reason.to_string());
                    format!("Transaction has been rejected: {}", reason)
                }
            },
            CliError::AskarError(askar_error) => match askar_error.kind() {
                AskarErrorKind::Backend => {
                    format!("Wallet error occurred \"{}\".", askar_error.to_string())
                }
                AskarErrorKind::Busy => {
                    format!("Unable to query wallet \"{}\".", extra.unwrap_or_default())
                }
                AskarErrorKind::Duplicate => format!("Record already exist in the wallet"),
                AskarErrorKind::Encryption => format!(
                    "Invalid key provided for the wallet \"{}\"",
                    extra.unwrap_or_default()
                ),
                AskarErrorKind::Input => format!(
                    "Invalid configuration provided for the wallet: {}",
                    askar_error.message().unwrap_or_default()
                ),
                AskarErrorKind::NotFound => askar_error.to_string(),
                AskarErrorKind::Custom | AskarErrorKind::Unexpected => format!(
                    "Unexpected wallet error occurred \"{}\"",
                    askar_error.message().unwrap_or_default()
                ),
                AskarErrorKind::Unsupported => {
                    askar_error.message().unwrap_or_default().to_string()
                }
            },
            CliError::FileSystemError(fs_error) => fs_error.to_string(),
        }
    }
}
