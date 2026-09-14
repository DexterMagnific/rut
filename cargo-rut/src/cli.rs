use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cargo-rut", version, about = "Run rut test suites")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run test suites selected by path or typename
    Run(RunArgs),

    /// List discovered suite files, names, and typenames
    List(ListArgs),
}

#[derive(Parser)]
pub struct ListArgs {
    /// Path to suite file (or directory for discovery)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

#[derive(Parser)]
pub struct RunArgs {
    /// Path to suite file (or directory for discovery)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Exact suite typename to run
    #[arg(long, value_name = "TYPE")]
    pub typename: Option<String>,

    /// Runner type: parallel or sequential
    #[arg(long, value_enum, default_value = "parallel")]
    pub runner: RunnerType,

    /// Max concurrent jobs (parallel runner only)
    #[arg(long, short = 'j')]
    pub jobs: Option<usize>,

    /// Shuffle test case order (parallel runner only)
    #[arg(long)]
    pub shuffle: bool,

    /// Run tests whose qualified suite.case.test name contains this value
    #[arg(long, value_name = "PATTERN")]
    pub filter: Vec<String>,

    /// Stop admitting new test cases after the first failure
    #[arg(long)]
    pub fail_fast: bool,

    /// Write JUnit XML for a single selected suite
    #[arg(long, value_name = "FILE", conflicts_with = "junit_dir")]
    pub junit: Option<PathBuf>,

    /// Write one predictable JUnit XML file per selected suite
    #[arg(long, value_name = "DIR", conflicts_with = "junit")]
    pub junit_dir: Option<PathBuf>,

    /// Write GoogleTest JSON for a single selected suite
    #[arg(long, value_name = "FILE", conflicts_with = "gtest_dir")]
    pub gtest: Option<PathBuf>,

    /// Write one predictable GoogleTest JSON file per selected suite
    #[arg(long, value_name = "DIR", conflicts_with = "gtest")]
    pub gtest_dir: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum RunnerType {
    Parallel,
    Sequential,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(arguments: &[&str]) -> RunArgs {
        let cli = Cli::try_parse_from(arguments).unwrap();
        match cli.command {
            Commands::Run(args) => args,
            Commands::List(_) => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_legacy_path_syntax() {
        let args = run_args(&["cargo-rut", "run", "tests/suite.rs"]);

        assert_eq!(args.path, Some(PathBuf::from("tests/suite.rs")));
        assert_eq!(args.typename, None);
        assert!(matches!(args.runner, RunnerType::Parallel));
        assert_eq!(args.jobs, None);
        assert!(!args.shuffle);
        assert!(args.filter.is_empty());
        assert!(!args.fail_fast);
        assert_eq!(args.junit, None);
        assert_eq!(args.junit_dir, None);
        assert_eq!(args.gtest, None);
        assert_eq!(args.gtest_dir, None);
    }

    #[test]
    fn parses_typename_without_a_path() {
        let args = run_args(&["cargo-rut", "run", "--typename", "CalculatorSuite"]);

        assert_eq!(args.path, None);
        assert_eq!(args.typename.as_deref(), Some("CalculatorSuite"));
    }

    #[test]
    fn parses_path_scoped_typename_with_runner_options() {
        let args = run_args(&[
            "cargo-rut",
            "run",
            "tests",
            "--typename",
            "CalculatorSuite",
            "--runner",
            "sequential",
            "--jobs",
            "2",
            "--shuffle",
            "--filter",
            "addition",
            "--filter",
            "edge case",
            "--fail-fast",
        ]);

        assert_eq!(args.path, Some(PathBuf::from("tests")));
        assert_eq!(args.typename.as_deref(), Some("CalculatorSuite"));
        assert!(matches!(args.runner, RunnerType::Sequential));
        assert_eq!(args.jobs, Some(2));
        assert!(args.shuffle);
        assert_eq!(args.filter, ["addition", "edge case"]);
        assert!(args.fail_fast);
    }

    #[test]
    fn parses_junit_file_and_directory_outputs() {
        let file = run_args(&[
            "cargo-rut",
            "run",
            "suite.rs",
            "--junit",
            "reports/suite.xml",
        ]);
        assert_eq!(file.junit, Some(PathBuf::from("reports/suite.xml")));

        let directory = run_args(&["cargo-rut", "run", "tests", "--junit-dir", "reports"]);
        assert_eq!(directory.junit_dir, Some(PathBuf::from("reports")));
    }

    #[test]
    fn rejects_conflicting_junit_outputs() {
        let error = match Cli::try_parse_from([
            "cargo-rut",
            "run",
            "--junit",
            "report.xml",
            "--junit-dir",
            "reports",
        ]) {
            Ok(_) => panic!("conflicting JUnit options should be rejected"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn parses_gtest_outputs_and_allows_junit_too() {
        let args = run_args(&[
            "cargo-rut",
            "run",
            "suite.rs",
            "--junit",
            "report.xml",
            "--gtest",
            "report.json",
        ]);

        assert_eq!(args.junit, Some(PathBuf::from("report.xml")));
        assert_eq!(args.gtest, Some(PathBuf::from("report.json")));

        let directory = run_args(&["cargo-rut", "run", "tests", "--gtest-dir", "reports"]);
        assert_eq!(directory.gtest_dir, Some(PathBuf::from("reports")));
    }

    #[test]
    fn rejects_conflicting_gtest_outputs() {
        let error = Cli::try_parse_from([
            "cargo-rut",
            "run",
            "--gtest",
            "report.json",
            "--gtest-dir",
            "reports",
        ])
        .err()
        .expect("conflicting GTest options should be rejected");

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn parses_list_with_an_optional_path() {
        let cli = Cli::try_parse_from(["cargo-rut", "list", "tests"]).unwrap();

        match cli.command {
            Commands::List(args) => assert_eq!(args.path, Some(PathBuf::from("tests"))),
            Commands::Run(_) => panic!("expected list command"),
        }
    }
}
