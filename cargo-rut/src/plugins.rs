use crate::error::RutError;
use std::path::{Path, PathBuf};

/// A crate exporting runner or reporter plugins through `rut::export_plugins!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCrate {
    /// Package name as declared in the plugin crate manifest.
    pub package: String,
    /// Absolute path of the crate directory.
    pub path: PathBuf,
}

impl PluginCrate {
    /// The identifier used to call the crate entry point in generated code.
    pub fn ident(&self) -> String {
        self.package.replace('-', "_")
    }
}

/// Resolves explicit plugin crates and every crate found in the scanned directories.
pub fn resolve(crates: &[PathBuf], directories: &[PathBuf]) -> Result<Vec<PluginCrate>, RutError> {
    let mut resolved = Vec::new();

    for path in crates {
        push_unique(&mut resolved, load_crate(path)?);
    }

    for directory in directories {
        for path in child_crates(directory)? {
            push_unique(&mut resolved, load_crate(&path)?);
        }
    }

    Ok(resolved)
}

fn push_unique(resolved: &mut Vec<PluginCrate>, candidate: PluginCrate) {
    if !resolved
        .iter()
        .any(|existing| existing.path == candidate.path)
    {
        resolved.push(candidate);
    }
}

fn child_crates(directory: &Path) -> Result<Vec<PathBuf>, RutError> {
    let entries = std::fs::read_dir(directory).map_err(|error| {
        RutError::PluginError(format!(
            "failed to read plugin directory {}: {error}",
            directory.display()
        ))
    })?;

    let mut crates = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| {
                RutError::PluginError(format!(
                    "failed to read plugin directory {}: {error}",
                    directory.display()
                ))
            })?
            .path();

        if path.join("Cargo.toml").is_file() {
            crates.push(path);
        }
    }

    crates.sort();
    Ok(crates)
}

fn load_crate(path: &Path) -> Result<PluginCrate, RutError> {
    let path = std::fs::canonicalize(path).map_err(|error| {
        RutError::PluginError(format!(
            "failed to resolve plugin crate {}: {error}",
            path.display()
        ))
    })?;
    let manifest_path = path.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|error| {
        RutError::PluginError(format!(
            "failed to read {}: {error}",
            manifest_path.display()
        ))
    })?;
    let package = package_name(&manifest).ok_or_else(|| {
        RutError::PluginError(format!(
            "{} does not declare a [package] name",
            manifest_path.display()
        ))
    })?;

    Ok(PluginCrate { package, path })
}

/// Reads `name` from the `[package]` table without pulling in a TOML parser.
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();

        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }

        if in_package && let Some((key, value)) = line.split_once('=') {
            if key.trim() != "name" {
                continue;
            }
            return Some(value.trim().trim_matches('"').to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_crate(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
        )
        .unwrap();
        path
    }

    #[test]
    fn reads_the_package_name() {
        assert_eq!(
            package_name("[package]\nname = \"my-plugins\" # comment\n").as_deref(),
            Some("my-plugins")
        );
        assert_eq!(
            package_name("[dependencies]\nname = \"not-a-package\"\n"),
            None
        );
    }

    #[test]
    fn converts_package_names_to_identifiers() {
        let plugin = PluginCrate {
            package: "my-plugins".to_string(),
            path: PathBuf::new(),
        };

        assert_eq!(plugin.ident(), "my_plugins");
    }

    #[test]
    fn resolves_directories_and_deduplicates() {
        let root = TempDir::new().unwrap();
        let first = write_crate(root.path(), "first");
        write_crate(root.path(), "second");
        std::fs::create_dir_all(root.path().join("not-a-crate")).unwrap();

        let resolved = resolve(&[first], &[root.path().to_path_buf()]).unwrap();

        assert_eq!(
            resolved
                .iter()
                .map(|plugin| plugin.package.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[test]
    fn reports_missing_plugin_crates() {
        let error = resolve(&[PathBuf::from("does/not/exist")], &[]).unwrap_err();

        assert!(error.to_string().contains("failed to resolve plugin crate"));
    }
}
