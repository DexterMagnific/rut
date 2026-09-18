use crate::cli::args::{ArgSpec, PluginArgs};
use crate::reporter::TestReporter;
use crate::runner::{BoxedRunner, TestRunner};
use anyhow::Result;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// Arguments that apply to every runner, whichever one is selected.
#[derive(Clone, Debug, Default)]
pub struct CoreArgs {
    pub filters: Vec<String>,
    pub fail_fast: bool,
}

/// Identity of the suite the harness was generated for.
///
/// `cargo-rut` fills these values through the environment, so reporters can
/// derive per-suite output paths without the driver computing them.
#[derive(Clone, Debug, Default)]
pub struct SuiteContext {
    pub typename: String,
    pub slug: String,
    pub source_file: Option<PathBuf>,
    /// Suite directory relative to the discovery scope, used to mirror layouts.
    pub relative_dir: PathBuf,
}

impl SuiteContext {
    pub fn from_env() -> Self {
        let typename = std::env::var("RUT_SUITE_TYPENAME").unwrap_or_default();
        let slug = std::env::var("RUT_SUITE_SLUG")
            .ok()
            .filter(|slug| !slug.is_empty())
            .unwrap_or_else(|| default_slug(&typename));

        Self {
            typename,
            slug,
            source_file: std::env::var_os("RUT_SUITE_FILE").map(PathBuf::from),
            relative_dir: std::env::var_os("RUT_SUITE_REL_DIR")
                .map(PathBuf::from)
                .unwrap_or_default(),
        }
    }

    /// Returns `<directory>/<relative dir>/<slug>.<extension>`.
    pub fn mirrored_path(&self, directory: &Path, extension: &str) -> PathBuf {
        directory
            .join(&self.relative_dir)
            .join(format!("{}.{extension}", self.slug))
    }
}

fn default_slug(typename: &str) -> String {
    let slug = typename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = slug.trim_matches('-').to_string();

    if slug.is_empty() {
        "suite".to_string()
    } else {
        slug
    }
}

/// Makes a runner selectable through `--runner <NAME>`.
///
/// The name and the argument declarations are associated items, so the registry
/// reads them without building the runner first.
pub trait RunnerPlugin: TestRunner + Sized + 'static {
    /// The name used to select this runner on the command line.
    const NAME: &'static str;

    /// One line description shown in `--help`.
    const ABOUT: &'static str = "";

    /// The arguments this runner adds to the command line.
    fn args() -> Vec<ArgSpec> {
        Vec::new()
    }

    /// Builds the runner from its own arguments and the runner-agnostic ones.
    fn from_args(args: &PluginArgs<'_>, core: &CoreArgs) -> Result<Self>;
}

/// Makes a reporter selectable through `--reporters <NAMES>`.
pub trait ReporterPlugin: TestReporter + Sized + 'static {
    /// The name used to select this reporter on the command line.
    const NAME: &'static str;

    /// One line description shown in `--help`.
    const ABOUT: &'static str = "";

    /// The arguments this reporter adds to the command line.
    fn args() -> Vec<ArgSpec> {
        Vec::new()
    }

    /// Whether the reporter should run even though it was not named explicitly.
    ///
    /// Reporters that own an output path usually activate when that path is
    /// given, so `--junit-dir target/junit` is enough to enable them.
    fn is_active(_args: &PluginArgs<'_>) -> bool {
        false
    }

    /// Builds the reporter from its own arguments and the suite identity.
    fn from_args(args: &PluginArgs<'_>, suite: &SuiteContext) -> Result<Self>;
}

/// Object-safe view of a [`RunnerPlugin`], stored by the registry.
///
/// Associated items cannot be reached through a trait object, so the registry
/// holds this view instead. It is implemented automatically for every plugin.
pub trait RunnerPluginInternal: Send + Sync {
    fn name(&self) -> &'static str;
    fn about(&self) -> &'static str;
    fn args(&self) -> Vec<ArgSpec>;
    fn build(&self, args: &PluginArgs<'_>, core: &CoreArgs) -> Result<BoxedRunner>;
}

/// Object-safe view of a [`ReporterPlugin`], stored by the registry.
pub trait ReporterPluginInternal: Send + Sync {
    fn name(&self) -> &'static str;
    fn about(&self) -> &'static str;
    fn args(&self) -> Vec<ArgSpec>;
    fn is_active(&self, args: &PluginArgs<'_>) -> bool;
    fn build(&self, args: &PluginArgs<'_>, suite: &SuiteContext) -> Result<Box<dyn TestReporter>>;
}

/// Turns the associated items of a plugin into a value the registry can hold.
pub(crate) struct Plugin<T>(PhantomData<fn() -> T>);

impl<T> Plugin<T> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: RunnerPlugin> RunnerPluginInternal for Plugin<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn about(&self) -> &'static str {
        T::ABOUT
    }

    fn args(&self) -> Vec<ArgSpec> {
        T::args()
    }

    fn build(&self, args: &PluginArgs<'_>, core: &CoreArgs) -> Result<BoxedRunner> {
        Ok(Box::new(T::from_args(args, core)?))
    }
}

impl<T: ReporterPlugin> ReporterPluginInternal for Plugin<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn about(&self) -> &'static str {
        T::ABOUT
    }

    fn args(&self) -> Vec<ArgSpec> {
        T::args()
    }

    fn is_active(&self, args: &PluginArgs<'_>) -> bool {
        T::is_active(args)
    }

    fn build(&self, args: &PluginArgs<'_>, suite: &SuiteContext) -> Result<Box<dyn TestReporter>> {
        Ok(Box::new(T::from_args(args, suite)?))
    }
}
