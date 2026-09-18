use crate::error::RutError;
use clap::{Parser, Subcommand};
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
    ///
    /// Arguments that are not driver options are forwarded to the test
    /// harness, where the selected runner and reporters parse them.
    #[command(disable_help_flag = true)]
    Run {
        #[arg(
            value_name = "ARGS",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },

    /// List discovered suite files, names, and typenames
    List(ListArgs),
}

#[derive(Parser)]
pub struct ListArgs {
    /// Path to suite file (or directory for discovery)
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

/// Driver options of `cargo rut run`, plus the arguments forwarded verbatim.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunArgs {
    pub path: Option<PathBuf>,
    pub typename: Option<String>,
    /// Directories of crates exporting runner or reporter plugins.
    pub plugin_crates: Vec<PathBuf>,
    /// Directories scanned for plugin crates.
    pub plugin_dirs: Vec<PathBuf>,
    /// Whether help was requested; it is forwarded to the harness as well.
    pub help: bool,
    pub harness_args: Vec<String>,
}

pub const RUN_HELP: &str = "\
Run rut test suites

Usage: cargo rut run [PATH] [DRIVER OPTIONS] [HARNESS OPTIONS]

Arguments:
  [PATH]  Suite file, or directory to discover suites in

Driver options:
      --typename <TYPE>     Exact suite typename to run
      --plugin-crate <DIR>  Crate exporting runner or reporter plugins, repeatable
      --plugins-dir <DIR>   Directory scanned for plugin crates, repeatable
  -h, --help                Show this help, followed by the harness help

Every other argument is forwarded to the test harness, where the selected runner
and reporters parse it. Use `--` to force forwarding.
";

impl RunArgs {
    /// Splits raw `cargo rut run` arguments into driver options and forwarded ones.
    ///
    /// Only the first bare token is treated as the suite path; later bare tokens
    /// are assumed to be values of forwarded options, whose arity is unknown here.
    pub fn parse(tokens: &[String]) -> Result<Self, RutError> {
        let mut args = Self::default();
        let mut positional_allowed = true;
        let mut iterator = tokens.iter().enumerate();

        while let Some((index, token)) = iterator.next() {
            if token == "--" {
                args.harness_args
                    .extend(tokens[index + 1..].iter().cloned());
                break;
            }

            match split_option(token) {
                Some(("--typename", inline)) => {
                    args.typename = Some(option_value("--typename", inline, &mut iterator)?);
                }
                Some(("--plugin-crate", inline)) => {
                    args.plugin_crates.push(PathBuf::from(option_value(
                        "--plugin-crate",
                        inline,
                        &mut iterator,
                    )?));
                }
                Some(("--plugins-dir", inline)) => {
                    args.plugin_dirs.push(PathBuf::from(option_value(
                        "--plugins-dir",
                        inline,
                        &mut iterator,
                    )?));
                }
                _ => {
                    if token == "--help" || token == "-h" {
                        args.help = true;
                    }

                    if positional_allowed && !token.starts_with('-') {
                        args.path = Some(PathBuf::from(token));
                    } else {
                        args.harness_args.push(token.clone());
                    }
                    positional_allowed = false;
                }
            }
        }

        Ok(args)
    }
}

fn split_option(token: &str) -> Option<(&str, Option<&str>)> {
    if !token.starts_with("--") {
        return None;
    }

    match token.split_once('=') {
        Some((name, value)) => Some((name, Some(value))),
        None => Some((token, None)),
    }
}

fn option_value<'a>(
    name: &str,
    inline: Option<&str>,
    iterator: &mut impl Iterator<Item = (usize, &'a String)>,
) -> Result<String, RutError> {
    if let Some(value) = inline {
        return Ok(value.to_string());
    }

    iterator
        .next()
        .map(|(_, value)| value.clone())
        .ok_or_else(|| RutError::InvalidArguments(format!("{name} requires a value")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> RunArgs {
        let tokens = arguments
            .iter()
            .map(|argument| argument.to_string())
            .collect::<Vec<_>>();
        RunArgs::parse(&tokens).unwrap()
    }

    #[test]
    fn parses_path_only() {
        let args = parse(&["tests/suite.rs"]);

        assert_eq!(args.path, Some(PathBuf::from("tests/suite.rs")));
        assert_eq!(args.typename, None);
        assert!(args.harness_args.is_empty());
    }

    #[test]
    fn forwards_unknown_options_with_their_values() {
        let args = parse(&["tests", "--runner", "sequential", "--jobs", "4", "--shuffle"]);

        assert_eq!(args.path, Some(PathBuf::from("tests")));
        assert_eq!(
            args.harness_args,
            ["--runner", "sequential", "--jobs", "4", "--shuffle"]
        );
    }

    #[test]
    fn accepts_driver_options_before_the_path() {
        let args = parse(&[
            "--typename",
            "CalculatorSuite",
            "tests",
            "--filter",
            "addition",
        ]);

        assert_eq!(args.typename.as_deref(), Some("CalculatorSuite"));
        assert_eq!(args.path, Some(PathBuf::from("tests")));
        assert_eq!(args.harness_args, ["--filter", "addition"]);
    }

    #[test]
    fn accepts_inline_driver_values() {
        let args = parse(&["--typename=CalculatorSuite", "--plugin-crate=plugins/mine"]);

        assert_eq!(args.typename.as_deref(), Some("CalculatorSuite"));
        assert_eq!(args.plugin_crates, [PathBuf::from("plugins/mine")]);
    }

    #[test]
    fn collects_repeated_plugin_locations() {
        let args = parse(&[
            "--plugin-crate",
            "a",
            "--plugin-crate",
            "b",
            "--plugins-dir",
            "plugins",
        ]);

        assert_eq!(args.plugin_crates, [PathBuf::from("a"), PathBuf::from("b")]);
        assert_eq!(args.plugin_dirs, [PathBuf::from("plugins")]);
    }

    #[test]
    fn forwards_everything_after_a_double_dash() {
        let args = parse(&["suite.rs", "--", "--typename", "NotADriverOption"]);

        assert_eq!(args.path, Some(PathBuf::from("suite.rs")));
        assert_eq!(args.typename, None);
        assert_eq!(args.harness_args, ["--typename", "NotADriverOption"]);
    }

    #[test]
    fn records_and_forwards_help() {
        let args = parse(&["suite.rs", "--help"]);

        assert!(args.help);
        assert_eq!(args.harness_args, ["--help"]);
    }

    #[test]
    fn reports_missing_driver_values() {
        let error = RunArgs::parse(&["--typename".to_string()]).unwrap_err();

        assert!(error.to_string().contains("--typename requires a value"));
    }

    #[test]
    fn collects_raw_run_arguments() {
        let cli = Cli::try_parse_from(["cargo-rut", "run", "tests", "--jobs", "4"]).unwrap();

        match cli.command {
            Commands::Run { args } => assert_eq!(args, ["tests", "--jobs", "4"]),
            Commands::List(_) => panic!("expected run command"),
        }
    }

    #[test]
    fn parses_list_with_an_optional_path() {
        let cli = Cli::try_parse_from(["cargo-rut", "list", "tests"]).unwrap();

        match cli.command {
            Commands::List(args) => assert_eq!(args.path, Some(PathBuf::from("tests"))),
            Commands::Run { .. } => panic!("expected list command"),
        }
    }
}
