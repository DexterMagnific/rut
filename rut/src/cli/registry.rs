use crate::cli::builtin;
use crate::cli::plugin::{
    Plugin, ReporterPlugin, ReporterPluginInternal, RunnerPlugin, RunnerPluginInternal,
};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

/// The runners and reporters a harness can select between.
///
/// Built-in plugins are registered by [`PluginRegistry::with_builtins`]; plugin
/// crates add their own through [`register_runner`](Self::register_runner) and
/// [`register_reporter`](Self::register_reporter).
pub struct PluginRegistry {
    runners: Vec<Box<dyn RunnerPluginInternal>>,
    reporters: Vec<Box<dyn ReporterPluginInternal>>,
    default_runner: String,
    default_reporter: String,
}

impl PluginRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            runners: Vec::new(),
            reporters: Vec::new(),
            default_runner: String::new(),
            default_reporter: String::new(),
        }
    }

    /// Creates a registry holding the runners and reporters shipped with `rut`.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        builtin::register(&mut registry);
        registry
    }

    pub fn register_runner<T: RunnerPlugin>(&mut self) -> &mut Self {
        self.runners.push(Box::new(Plugin::<T>::new()));
        self
    }

    pub fn register_reporter<T: ReporterPlugin>(&mut self) -> &mut Self {
        self.reporters.push(Box::new(Plugin::<T>::new()));
        self
    }

    /// Sets the runner used when `--runner` is omitted.
    pub fn set_default_runner(&mut self, name: impl Into<String>) -> &mut Self {
        self.default_runner = name.into();
        self
    }

    /// Sets the reporter used when `--reporters` is omitted.
    pub fn set_default_reporter(&mut self, name: impl Into<String>) -> &mut Self {
        self.default_reporter = name.into();
        self
    }

    pub fn default_runner(&self) -> &str {
        &self.default_runner
    }

    pub fn default_reporter(&self) -> &str {
        &self.default_reporter
    }

    pub fn runners(&self) -> &[Box<dyn RunnerPluginInternal>] {
        &self.runners
    }

    pub fn reporters(&self) -> &[Box<dyn ReporterPluginInternal>] {
        &self.reporters
    }

    pub fn runner(&self, name: &str) -> Option<&dyn RunnerPluginInternal> {
        self.runners
            .iter()
            .find(|plugin| plugin.name() == name)
            .map(|plugin| plugin.as_ref())
    }

    pub fn reporter(&self, name: &str) -> Option<&dyn ReporterPluginInternal> {
        self.reporters
            .iter()
            .find(|plugin| plugin.name() == name)
            .map(|plugin| plugin.as_ref())
    }

    /// Rejects duplicate plugin names and colliding argument declarations.
    pub fn validate(&self) -> Result<()> {
        let mut problems = Vec::new();

        check_unique_names(
            "runner",
            self.runners.iter().map(|plugin| plugin.name()),
            &mut problems,
        );
        check_unique_names(
            "reporter",
            self.reporters.iter().map(|plugin| plugin.name()),
            &mut problems,
        );

        let mut longs = HashMap::<String, String>::new();
        let mut shorts = HashMap::<char, String>::new();

        for (owner, specs) in self
            .runners
            .iter()
            .map(|plugin| (format!("runner '{}'", plugin.name()), plugin.args()))
            .chain(
                self.reporters
                    .iter()
                    .map(|plugin| (format!("reporter '{}'", plugin.name()), plugin.args())),
            )
        {
            for spec in specs {
                if let Some(previous) = longs.insert(spec.long.clone(), owner.clone()) {
                    problems.push(format!(
                        "--{} is declared by {previous} and {owner}",
                        spec.long
                    ));
                }
                if let Some(short) = spec.short
                    && let Some(previous) = shorts.insert(short, owner.clone())
                {
                    problems.push(format!("-{short} is declared by {previous} and {owner}"));
                }
            }
        }

        for long in [
            CORE_RUNNER,
            CORE_REPORTER,
            CORE_FILTER,
            CORE_FAIL_FAST,
            CORE_SUITE_ARGS,
        ] {
            if let Some(owner) = longs.get(long) {
                problems.push(format!("--{long} is reserved but is declared by {owner}"));
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "invalid plugin registry:\n  {}",
                problems.join("\n  ")
            ))
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

pub(crate) const CORE_RUNNER: &str = "runner";
pub(crate) const CORE_REPORTER: &str = "reporters";
pub(crate) const CORE_FILTER: &str = "filter";
pub(crate) const CORE_FAIL_FAST: &str = "fail-fast";
pub(crate) const CORE_SUITE_ARGS: &str = "suite-args";

fn check_unique_names<'a>(
    kind: &str,
    names: impl Iterator<Item = &'a str>,
    problems: &mut Vec<String>,
) {
    let mut seen = Vec::new();
    for name in names {
        if seen.contains(&name) {
            problems.push(format!("duplicate {kind} plugin name '{name}'"));
        } else {
            seen.push(name);
        }
    }
}
