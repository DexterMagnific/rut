use crate::error::RutError;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn discover_suite_file(path: Option<PathBuf>) -> Result<PathBuf, RutError> {
    let start = path.unwrap_or_else(|| PathBuf::from("."));

    if start.is_file() {
        return Ok(start);
    }

    for entry in WalkDir::new(&start).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "rs") {
            let content = std::fs::read_to_string(path)
                .map_err(|e| RutError::ParseError(format!("failed to read {}: {}", path.display(), e)))?;
            if content.contains("suite!") {
                return Ok(path.to_path_buf());
            }
        }
    }

    Err(RutError::NoSuiteFileFound(start))
}