//! Command line plumbing for generated test harnesses.
//!
//! Runners and reporters are exposed as *plugins*: each one declares the
//! arguments it understands, and the harness merges those declarations into a
//! single command line. `cargo-rut` therefore forwards arguments unchanged
//! instead of knowing about every option itself.
//!
//! ```no_run
//! # use rut::cli::PluginRegistry;
//! # use rut::TestSuite;
//! # async fn example(make_suite: fn() -> Box<dyn TestSuite>) {
//! let registry = PluginRegistry::with_builtins();
//! rut::cli::harness_main(registry, make_suite).await;
//! # }
//! ```

mod args;
pub(crate) mod builtin;
mod harness;
mod plugin;
mod registry;

pub use args::{ArgKind, ArgSpec, PluginArgs};
pub use harness::{build_command, harness_main, harness_main_from, harness_status};
pub use plugin::{
    CoreArgs, ReporterPlugin, ReporterPluginInternal, RunnerPlugin, RunnerPluginInternal,
    SuiteContext,
};
pub use registry::PluginRegistry;

/// Declares the plugins a crate exports to `cargo-rut`.
///
/// The macro generates a `__rut_plugins` entry point at the crate root, which
/// the generated harness calls explicitly. An explicit call keeps the
/// registrations alive even when nothing else references the plugin crate.
///
/// ```
/// # use rut::cli::{CoreArgs, PluginArgs, RunnerPlugin};
/// # use rut::SequentialRunner;
/// struct MyRunner(SequentialRunner);
/// # #[rut::__private::async_trait]
/// # impl rut::TestRunner for MyRunner {
/// #     fn set_suite(&mut self, suite: Box<dyn rut::TestSuite>) { self.0.set_suite(suite) }
/// #     fn set_reporter(&mut self, r: Box<dyn rut::TestReporter>) { self.0.set_reporter(r) }
/// #     async fn run(self) -> rut::ReporterResult<rut::SuiteReport> { self.0.run().await }
/// # }
///
/// impl RunnerPlugin for MyRunner {
///     const NAME: &'static str = "mine";
///
///     fn from_args(_args: &PluginArgs<'_>, _core: &CoreArgs) -> anyhow::Result<Self> {
///         Ok(MyRunner(SequentialRunner::new()))
///     }
/// }
///
/// rut::export_plugins! {
///     runners: [MyRunner],
/// }
/// ```
#[macro_export]
macro_rules! export_plugins {
    (
        $(runners: [$($runner:ty),* $(,)?] $(,)?)?
        $(reporters: [$($reporter:ty),* $(,)?] $(,)?)?
    ) => {
        /// Registers the plugins this crate exports. Called by generated harnesses.
        pub fn __rut_plugins(registry: &mut $crate::cli::PluginRegistry) {
            $($(registry.register_runner::<$runner>();)*)?
            $($(registry.register_reporter::<$reporter>();)*)?
        }
    };
}
