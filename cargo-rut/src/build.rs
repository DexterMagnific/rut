use crate::error::RutError;
use cargo_metadata::{DependencyKind, MetadataCommand};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "run-suite"
version = "0.1.0"
edition = "2024"

[dependencies]
rut = { path = "{rut_path}"{rut_options} }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[workspace]
"#;

struct RutDependency {
    path: PathBuf,
    features: Vec<String>,
    uses_default_features: bool,
    owner_id: String,
    target_directory: PathBuf,
}

pub struct CachedProject {
    dir: PathBuf,
    target_dir: PathBuf,
}

impl CachedProject {
    pub fn new(suite_file: &Path, typename: &str) -> Result<Self, RutError> {
        let dependency = resolve_rut_dependency(suite_file)?;
        let suite_file = std::fs::canonicalize(suite_file).map_err(|error| {
            RutError::CacheError(format!(
                "failed to resolve {}: {error}",
                suite_file.display()
            ))
        })?;
        let suite_id = suite_id(typename, &suite_file, &dependency.owner_id);
        let cache_root = dependency.target_directory.join("cargo-rut");
        let dir = cache_root.join("generated").join(&suite_id);
        let target_dir = cache_root.join(&suite_id);

        std::fs::create_dir_all(dir.join("src")).map_err(|error| {
            RutError::CacheError(format!(
                "failed to create cache directory {}: {error}",
                dir.display()
            ))
        })?;

        let rut_path = dependency.path.to_string_lossy().replace('\\', "/");
        let escaped_rut_path = rut_path.replace('\\', "\\\\").replace('"', "\\\"");
        let mut options = String::new();
        if !dependency.uses_default_features {
            options.push_str(", default-features = false");
        }
        if !dependency.features.is_empty() {
            let features = dependency
                .features
                .iter()
                .map(|feature| format!("\"{}\"", feature.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", ");
            options.push_str(&format!(", features = [{}]", features));
        }

        let cargo_toml = CARGO_TOML_TEMPLATE
            .replace("{rut_path}", &escaped_rut_path)
            .replace("{rut_options}", &options);

        write_if_changed(&dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

        Ok(Self { dir, target_dir })
    }

    pub fn write_wrapper(&self, content: &str) -> Result<bool, RutError> {
        write_if_changed(&self.dir.join("src/main.rs"), content.as_bytes())
    }

    pub fn build_and_run(&self) -> Result<std::process::ExitStatus, RutError> {
        let manifest_path = self.dir.join("Cargo.toml");
        let status = Command::new("cargo")
            .env("CARGO_TARGET_DIR", &self.target_dir)
            .args([
                "run",
                "--release",
                "--manifest-path",
                manifest_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| RutError::BuildError(format!("failed to run cargo: {}", e)))?;

        Ok(status)
    }
}

fn suite_id(typename: &str, suite_file: &Path, owner_id: &str) -> String {
    format!(
        "{}-{}",
        suite_slug(typename),
        project_hash(suite_file, owner_id)
    )
}

fn project_hash(suite_file: &Path, owner_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(owner_id.as_bytes());
    hasher.update([0]);
    hasher.update(suite_file.as_os_str().as_encoded_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn suite_slug(typename: &str) -> String {
    let characters = typename.chars().collect::<Vec<_>>();
    let mut slug = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if character.is_ascii_alphanumeric() {
            let previous = index.checked_sub(1).and_then(|index| characters.get(index));
            let next = characters.get(index + 1);
            let starts_word = character.is_ascii_uppercase()
                && (previous
                    .is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
                    || (previous.is_some_and(|value| value.is_ascii_uppercase())
                        && next.is_some_and(|value| value.is_ascii_lowercase())));

            if starts_word && !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }

    let mut slug = slug.trim_matches('-').chars().take(32).collect::<String>();
    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "suite".to_string()
    } else {
        slug
    }
}

fn write_if_changed(path: &Path, content: &[u8]) -> Result<bool, RutError> {
    if std::fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(false);
    }

    std::fs::write(path, content).map_err(|error| {
        RutError::CacheError(format!("failed to write {}: {error}", path.display()))
    })?;
    Ok(true)
}

fn resolve_rut_dependency(suite_file: &Path) -> Result<RutDependency, RutError> {
    let suite_file = std::fs::canonicalize(suite_file).map_err(|error| {
        RutError::DependencyError(format!(
            "failed to resolve {}: {error}",
            suite_file.display()
        ))
    })?;
    let metadata = MetadataCommand::new()
        .current_dir(suite_file.parent().unwrap_or_else(|| Path::new(".")))
        .exec()
        .map_err(|error| RutError::DependencyError(error.to_string()))?;

    let owner = metadata
        .packages
        .iter()
        .filter(|package| {
            package
                .manifest_path
                .parent()
                .is_some_and(|root| suite_file.starts_with(root.as_std_path()))
        })
        .max_by_key(|package| package.manifest_path.components().count())
        .ok_or_else(|| {
            RutError::DependencyError(format!(
                "{} is not inside a Cargo package",
                suite_file.display()
            ))
        })?;

    if owner.name == "rut" {
        let path = owner
            .manifest_path
            .parent()
            .ok_or_else(|| {
                RutError::DependencyError("rut manifest has no parent directory".to_string())
            })?
            .as_std_path()
            .to_path_buf();
        return Ok(RutDependency {
            path,
            features: Vec::new(),
            uses_default_features: true,
            owner_id: owner.id.to_string(),
            target_directory: metadata.target_directory.as_std_path().to_path_buf(),
        });
    }

    let declaration = owner
        .dependencies
        .iter()
        .find(|dependency| {
            dependency.name == "rut"
                && matches!(
                    dependency.kind,
                    DependencyKind::Normal | DependencyKind::Development
                )
        })
        .ok_or_else(|| {
            RutError::DependencyError(format!(
                "package `{}` must declare `rut` in [dev-dependencies] or [dependencies]",
                owner.name
            ))
        })?;

    let dependency_name = declaration.rename.as_deref().unwrap_or(&declaration.name);
    let resolve = metadata.resolve.as_ref().ok_or_else(|| {
        RutError::DependencyError("Cargo metadata did not include a dependency graph".to_string())
    })?;
    let owner_node = resolve
        .nodes
        .iter()
        .find(|node| node.id == owner.id)
        .ok_or_else(|| {
            RutError::DependencyError(
                "owning package is absent from the dependency graph".to_string(),
            )
        })?;
    let dependency_node = owner_node
        .deps
        .iter()
        .find(|dependency| dependency.name == dependency_name)
        .ok_or_else(|| {
            RutError::DependencyError("declared `rut` dependency was not resolved".to_string())
        })?;
    let rut_package = metadata
        .packages
        .iter()
        .find(|package| package.id == dependency_node.pkg && package.name == "rut")
        .ok_or_else(|| {
            RutError::DependencyError("resolved dependency is not the `rut` package".to_string())
        })?;
    let path = rut_package
        .manifest_path
        .parent()
        .ok_or_else(|| {
            RutError::DependencyError("rut manifest has no parent directory".to_string())
        })?
        .as_std_path()
        .to_path_buf();

    Ok(RutDependency {
        path,
        features: declaration.features.clone(),
        uses_default_features: declaration.uses_default_features,
        owner_id: owner.id.to_string(),
        target_directory: metadata.target_directory.as_std_path().to_path_buf(),
    })
}

pub fn run_temp_project(
    suite_file: &Path,
    typename: &str,
    wrapper: String,
) -> Result<std::process::ExitStatus, RutError> {
    let project = CachedProject::new(suite_file, typename)?;
    project.write_wrapper(&wrapper)?;
    project.build_and_run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolves_rut_from_a_dev_dependency() {
        let project = TempDir::new().unwrap();
        let rut_path = project.path().join("framework");
        std::fs::create_dir_all(rut_path.join("src")).unwrap();
        std::fs::write(
            rut_path.join("Cargo.toml"),
            "[package]\nname = \"rut\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nexample-feature = []\n",
        )
        .unwrap();
        std::fs::write(rut_path.join("src/lib.rs"), "").unwrap();
        let escaped_path = rut_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        std::fs::write(
            project.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dev-dependencies]\nrut = {{ path = \"{escaped_path}\", default-features = false, features = [\"example-feature\"] }}\n"
            ),
        )
        .unwrap();
        let tests_dir = project.path().join("tests");
        std::fs::create_dir(&tests_dir).unwrap();
        let suite_file = tests_dir.join("suite.rs");
        std::fs::write(&suite_file, "").unwrap();

        let dependency = resolve_rut_dependency(&suite_file).unwrap();

        assert_eq!(dependency.path, std::fs::canonicalize(rut_path).unwrap());
        assert!(!dependency.uses_default_features);
        assert_eq!(dependency.features, ["example-feature"]);

        let cached_project = CachedProject::new(&suite_file, "CalculatorSuite").unwrap();
        let manifest = std::fs::read_to_string(cached_project.dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains("features = [\"example-feature\"]"));
        assert!(manifest.contains("name = \"run-suite\""));
    }

    #[test]
    fn reuses_the_project_and_does_not_rewrite_identical_wrapper() {
        let project = TempDir::new().unwrap();
        std::fs::create_dir_all(project.path().join("src")).unwrap();
        std::fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"rut\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(project.path().join("src/lib.rs"), "").unwrap();
        let suite_file = project.path().join("suite.rs");
        std::fs::write(&suite_file, "").unwrap();

        let first = CachedProject::new(&suite_file, "CalculatorSuite").unwrap();
        assert!(first.write_wrapper("static wrapper").unwrap());
        let second = CachedProject::new(&suite_file, "CalculatorSuite").unwrap();

        assert_eq!(first.dir, second.dir);
        assert_eq!(first.dir.file_name(), first.target_dir.file_name());
        assert_eq!(
            first.dir.parent().unwrap().file_name().unwrap(),
            "generated"
        );
        let directory_name = first.dir.file_name().unwrap().to_string_lossy();
        let hash = directory_name.rsplit('-').next().unwrap();
        assert!(directory_name.starts_with("calculator-suite-"));
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
        assert!(!second.write_wrapper("static wrapper").unwrap());
        assert!(second.write_wrapper("changed arguments").unwrap());
        assert_eq!(
            std::fs::read_to_string(second.dir.join("src/main.rs")).unwrap(),
            "changed arguments"
        );
    }

    #[test]
    fn creates_readable_suite_slugs() {
        assert_eq!(suite_slug("CalculatorSuite"), "calculator-suite");
        assert_eq!(suite_slug("HTTP2Suite"), "http2-suite");
        assert_eq!(suite_slug("Foo_Bar"), "foo-bar");
        assert_eq!(suite_slug("XMLParser2Test"), "xml-parser2-test");
        assert_eq!(suite_slug("__é__"), "suite");
        assert_eq!(suite_slug("_LeadingAndTrailing_"), "leading-and-trailing");
        assert!(suite_slug("AnExtremelyLongSuiteTypeNameThatNeedsTruncation").len() <= 32);
    }

    #[test]
    fn resolves_the_rut_package_without_a_self_dependency() {
        let project = TempDir::new().unwrap();
        std::fs::create_dir_all(project.path().join("src")).unwrap();
        std::fs::write(
            project.path().join("Cargo.toml"),
            "[package]\nname = \"rut\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(project.path().join("src/lib.rs"), "").unwrap();
        let suite_file = project.path().join("src/suite.rs");
        std::fs::write(&suite_file, "").unwrap();

        let dependency = resolve_rut_dependency(&suite_file).unwrap();

        assert_eq!(
            dependency.path,
            std::fs::canonicalize(project.path()).unwrap()
        );
    }
}
