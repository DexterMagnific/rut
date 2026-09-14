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
        ]);

        assert_eq!(args.path, Some(PathBuf::from("tests")));
        assert_eq!(args.typename.as_deref(), Some("CalculatorSuite"));
        assert!(matches!(args.runner, RunnerType::Sequential));
        assert_eq!(args.jobs, Some(2));
        assert!(args.shuffle);
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
