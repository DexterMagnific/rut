use anyhow::Error;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum RutError {
    #[error("no suite file found in {0}")]
    NoSuiteFileFound(PathBuf),

    #[error("typename not found in suite macro")]
    TypenameNotFound,

    #[error("failed to parse suite file: {0}")]
    ParseError(String),

    #[error("failed to create temp project: {0}")]
    TempProjectError(String),

    #[error("failed to build temp project: {0}")]
    BuildError(String),

    #[error("failed to run test suite: {0}")]
    RunError(String),
}