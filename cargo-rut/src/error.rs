use anyhow::Error;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum RutError {
    #[error("no suite file found in {0}")]
    NoSuiteFileFound(PathBuf),

    #[error("suite typename `{typename}` was not found in {scope}")]
    RequestedTypenameNotFound { typename: String, scope: PathBuf },

    #[error("suite typename `{typename}` is ambiguous; found in:\n{candidates}")]
    AmbiguousTypename {
        typename: String,
        candidates: String,
    },

    #[error("failed to parse suite file: {0}")]
    ParseError(String),

    #[error("failed to prepare runner cache: {0}")]
    CacheError(String),

    #[error("failed to resolve rut dependency: {0}")]
    DependencyError(String),

    #[error("failed to build temp project: {0}")]
    BuildError(String),

    #[error("failed to run test suite: {0}")]
    RunError(String),

    #[error("invalid report output: {0}")]
    ReportOutput(String),
}
