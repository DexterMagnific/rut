use crate::cli::args::{ArgSpec, PluginArgs};
use crate::cli::plugin::{ReporterPlugin, RunnerPlugin, SuiteContext};
use crate::cli::registry::PluginRegistry;
use crate::reporters::{GTestReporter, JUnitReporter, StdoutReporter};
use crate::runners::{ParallelRunner, SequentialRunner};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// Each plugin impl lives beside the runner or reporter it describes.
pub(crate) fn register(registry: &mut PluginRegistry) {
    registry
        .register_runner::<SequentialRunner>()
        .register_runner::<ParallelRunner>()
        .register_reporter::<StdoutReporter>()
        .register_reporter::<JUnitReporter>()
        .register_reporter::<GTestReporter>()
        .set_default_runner(ParallelRunner::NAME)
        .set_default_reporter(StdoutReporter::NAME);
}

/// Declares the `--<name>` and `--<name>-dir` pair shared by file reporters.
pub(crate) fn output_args(name: &str, label: &str) -> Vec<ArgSpec> {
    let directory = format!("{name}-dir");
    vec![
        ArgSpec::value(name.to_string())
            .value_name("FILE")
            .help(format!("Writes the {label} report to this file"))
            .conflicts_with(directory.clone()),
        ArgSpec::value(directory)
            .value_name("DIR")
            .help(format!(
                "Writes the {label} report under this directory, mirroring the suite path"
            ))
            .conflicts_with(name.to_string()),
    ]
}

pub(crate) fn output_path(
    args: &PluginArgs<'_>,
    suite: &SuiteContext,
    name: &str,
    extension: &str,
) -> Result<PathBuf> {
    let path = if let Some(file) = args.value(name) {
        PathBuf::from(file)
    } else if let Some(directory) = args.value(&format!("{name}-dir")) {
        suite.mirrored_path(Path::new(directory), extension)
    } else {
        PathBuf::from(format!("{}.{extension}", suite.slug))
    };

    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory {}", parent.display()))?;
    }

    Ok(path)
}
