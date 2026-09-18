use crate::cli::args::{PluginArgs, to_clap_arg};
use crate::cli::plugin::{CoreArgs, SuiteContext};
use crate::cli::registry::{
    CORE_FAIL_FAST, CORE_FILTER, CORE_REPORTER, CORE_RUNNER, PluginRegistry,
};
use crate::reporter::{MultiReporter, TestReporter};
use crate::suite::TestSuite;
use anyhow::{Result, anyhow};
use clap::builder::PossibleValuesParser;
use std::ffi::OsString;
use std::process::ExitCode;

const EXIT_FAILED_TESTS: u8 = 1;
const EXIT_ERROR: u8 = 2;

/// Runs a generated test harness from the process arguments.
///
/// Parses the core arguments plus every argument declared by the registered
/// plugins, builds the selected runner and reporters, and executes the suite.
/// The exit code is `0` when all tests pass, `1` when any test fails, and `2`
/// when the harness itself fails.
pub async fn harness_main<F>(registry: PluginRegistry, make_suite: F) -> ExitCode
where
    F: FnOnce() -> Box<dyn TestSuite>,
{
    harness_main_from(registry, make_suite, std::env::args_os()).await
}

/// Same as [`harness_main`] but with an explicit argument list.
pub async fn harness_main_from<F, I, T>(registry: PluginRegistry, make_suite: F, args: I) -> ExitCode
where
    F: FnOnce() -> Box<dyn TestSuite>,
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    ExitCode::from(harness_status(registry, make_suite, args).await)
}

/// Same as [`harness_main_from`] but returns the exit status as a number.
pub async fn harness_status<F, I, T>(registry: PluginRegistry, make_suite: F, args: I) -> u8
where
    F: FnOnce() -> Box<dyn TestSuite>,
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = match build_command(&registry) {
        Ok(command) => match command.try_get_matches_from(args) {
            Ok(matches) => matches,
            Err(error) => {
                let _ = error.print();
                return if error.use_stderr() { EXIT_ERROR } else { 0 };
            }
        },
        Err(error) => {
            eprintln!("rut: {error:#}");
            return EXIT_ERROR;
        }
    };

    match run(&registry, &matches, make_suite).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rut: {error:#}");
            EXIT_ERROR
        }
    }
}

/// Builds the command line interface exposed by a harness.
///
/// Exposed so drivers can render the same help text as the harness itself.
pub fn build_command(registry: &PluginRegistry) -> Result<clap::Command> {
    registry.validate()?;

    let suite = SuiteContext::from_env();
    let runner_names = registry
        .runners()
        .iter()
        .map(|plugin| plugin.name().to_string())
        .collect::<Vec<_>>();
    let reporter_names = registry
        .reporters()
        .iter()
        .map(|plugin| plugin.name().to_string())
        .collect::<Vec<_>>();

    let mut runner_arg = clap::Arg::new(CORE_RUNNER)
        .long(CORE_RUNNER)
        .value_name("NAME")
        .help("Execution strategy to use")
        .value_parser(PossibleValuesParser::new(runner_names));
    if !registry.default_runner().is_empty() {
        runner_arg = runner_arg.default_value(registry.default_runner().to_string());
    }

    let command = clap::Command::new("rut")
        .about(if suite.typename.is_empty() {
            "Runs a rut test suite".to_string()
        } else {
            format!("Runs the {} test suite", suite.typename)
        })
        .version(env!("CARGO_PKG_VERSION"))
        .arg(runner_arg)
        .arg(
            clap::Arg::new(CORE_REPORTER)
                .long(CORE_REPORTER)
                .value_name("NAMES")
                .action(clap::ArgAction::Set)
                .value_delimiter(',')
                .help("Comma separated output formats to produce")
                .value_parser(PossibleValuesParser::new(reporter_names)),
        )
        .arg(
            clap::Arg::new(CORE_FILTER)
                .long(CORE_FILTER)
                .value_name("PATTERN")
                .action(clap::ArgAction::Append)
                .help("Runs only tests whose Suite.case.test name contains the pattern"),
        )
        .arg(
            clap::Arg::new(CORE_FAIL_FAST)
                .long(CORE_FAIL_FAST)
                .action(clap::ArgAction::SetTrue)
                .help("Stops admitting new test cases after the first failure"),
        );

    let mut command = command;
    for plugin in registry.runners() {
        let heading = format!("Options for --runner {}", plugin.name());
        for spec in plugin.args() {
            command = command.arg(to_clap_arg(&spec).help_heading(heading.clone()));
        }
    }
    for plugin in registry.reporters() {
        let heading = format!("Options for --reporters {}", plugin.name());
        for spec in plugin.args() {
            command = command.arg(to_clap_arg(&spec).help_heading(heading.clone()));
        }
    }

    Ok(command)
}

async fn run<F>(registry: &PluginRegistry, matches: &clap::ArgMatches, make_suite: F) -> Result<u8>
where
    F: FnOnce() -> Box<dyn TestSuite>,
{
    let args = PluginArgs::new(matches);
    let core = CoreArgs {
        filters: args.values(CORE_FILTER).into_iter().map(String::from).collect(),
        fail_fast: args.flag(CORE_FAIL_FAST),
    };

    let runner_name = args
        .value(CORE_RUNNER)
        .ok_or_else(|| anyhow!("no runner selected and no default runner is registered"))?;
    let runner_plugin = registry
        .runner(runner_name)
        .ok_or_else(|| anyhow!("unknown runner '{runner_name}'"))?;

    let suite_context = SuiteContext::from_env();
    let reporter = build_reporter(registry, &args, &suite_context)?;

    let mut runner = runner_plugin.build(&args, &core)?;
    runner.set_suite(make_suite());
    runner.set_reporter(reporter);

    match runner.run_boxed().await {
        Ok(report) => Ok(if report.total_failed > 0 {
            EXIT_FAILED_TESTS
        } else {
            0
        }),
        Err(error) => {
            eprintln!("Reporting failed: {error}");
            Ok(EXIT_ERROR)
        }
    }
}

fn build_reporter(
    registry: &PluginRegistry,
    args: &PluginArgs<'_>,
    suite: &SuiteContext,
) -> Result<Box<dyn TestReporter>> {
    let mut selected = args
        .values(CORE_REPORTER)
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    if selected.is_empty() && !registry.default_reporter().is_empty() {
        selected.push(registry.default_reporter().to_string());
    }

    for plugin in registry.reporters() {
        let name = plugin.name().to_string();
        if !selected.contains(&name) && plugin.is_active(args) {
            selected.push(name);
        }
    }

    let mut reporters = Vec::new();
    for name in &selected {
        let plugin = registry
            .reporter(name)
            .ok_or_else(|| anyhow!("unknown reporter '{name}'"))?;
        reporters.push(plugin.build(args, suite)?);
    }

    match reporters.len() {
        0 => Err(anyhow!("no reporter selected")),
        1 => Ok(reporters.pop().expect("one reporter")),
        _ => Ok(Box::new(MultiReporter::new().with_reporters(reporters))),
    }
}
