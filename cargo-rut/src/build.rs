use crate::error::RutError;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "rut-temp-runner"
version = "0.1.0"
edition = "2024"

[dependencies]
rut = { path = "{rut_path}" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
"#;

pub struct TempProject {
    dir: TempDir,
}

impl TempProject {
    pub fn new(workspace_root: &Path) -> Result<Self, RutError> {
        let dir = TempDir::new().map_err(|e| RutError::TempProjectError(e.to_string()))?;

        // Calculate relative path from temp dir to workspace root
        let temp_path = dir.path();
        let rut_path = workspace_root.join("rut");
        let relative_rut_path = pathdiff::diff_paths(&rut_path, temp_path)
            .ok_or_else(|| RutError::TempProjectError("failed to compute relative path to rut".to_string()))?;

        let relative_rut_str = relative_rut_path.to_string_lossy().replace('\\', "/");

        let cargo_toml = CARGO_TOML_TEMPLATE.replace("{rut_path}", &relative_rut_str);

        let manifest_path = temp_path.join("Cargo.toml");
        std::fs::write(&manifest_path, cargo_toml)
            .map_err(|e| RutError::TempProjectError(format!("failed to write Cargo.toml: {}", e)))?;

        let src_dir = temp_path.join("src");
        std::fs::create_dir_all(&src_dir)
            .map_err(|e| RutError::TempProjectError(format!("failed to create src dir: {}", e)))?;

        Ok(Self { dir })
    }

    pub fn write_wrapper(&self, content: &str) -> Result<(), RutError> {
        let main_rs = self.dir.path().join("src/main.rs");
        std::fs::write(&main_rs, content)
            .map_err(|e| RutError::TempProjectError(format!("failed to write main.rs: {}", e)))?;
        Ok(())
    }

    pub fn build_and_run(&self) -> Result<std::process::ExitStatus, RutError> {
        let manifest_path = self.dir.path().join("Cargo.toml");
        let status = Command::new("cargo")
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

pub fn run_temp_project(
    workspace_root: &Path,
    wrapper: String,
) -> Result<std::process::ExitStatus, RutError> {
    let project = TempProject::new(workspace_root)?;
    project.write_wrapper(&wrapper)?;
    project.build_and_run()
}