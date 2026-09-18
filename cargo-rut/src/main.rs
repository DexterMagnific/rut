mod build;
mod cli;
mod discovery;
mod error;
mod parser;
mod plugins;
mod wrapper;

use crate::cli::{Cli, Commands, ListArgs, RUN_HELP, RunArgs};
use crate::discovery::{DiscoveredSuiteFile, discover_suites};
use crate::error::{Result, RutError};
use crate::plugins::PluginCrate;
use crate::wrapper::{WrapperOptions, generate_wrapper};
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ExecutionTarget {
    suite_file: PathBuf,
    typename: String,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(&args[1..]);

    let exit_code = match cli.command {
        Commands::Run { args } => run_command(RunArgs::parse(&args)?)?,        Commands::List(args) => list_command(args)?,
    };

    std::process::exit(exit_code);
}

fn list_command(args: ListArgs) -> Result<i32> {
    let discovered = discover_suites(args.path)?;
    print!("{}", format_suite_listing(&discovered));
    Ok(0)
}

fn format_suite_listing(discovered: &[DiscoveredSuiteFile]) -> String {
    let mut output = String::new();

    for suite_file in discovered {
        output.push_str(&format!("{}\n", suite_file.path.display()));
        for suite in &suite_file.suites {
            output.push_str(&format!(
                "  {} ({}): {} {}\n",
                suite.name,
                suite.typename,
                suite.cases.len(),
                pluralize(suite.cases.len(), "case", "cases")
            ));
            for case in &suite.cases {
                output.push_str(&format!(
                    "    {}: {} {}\n",
                    case.name,
                    case.test_count,
                    pluralize(case.test_count, "test", "tests")
                ));
            }
        }
    }

    output
}

fn pluralize(count: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}

fn run_command(args: RunArgs) -> Result<i32> {
    if args.help {
        print!("{RUN_HELP}");
    }

    let plugin_crates = plugins::resolve(&args.plugin_crates, &args.plugin_dirs)?;
    let discovered = match discover_suites(args.path.clone()) {
        Ok(discovered) => discovered,
        // Without a suite there is no harness help to append to the driver help.
        Err(_) if args.help => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let scope = args.path.as_deref().unwrap_or_else(|| Path::new("."));
    let targets = create_execution_targets(&discovered, args.typename.as_deref(), scope)?;

    // Harness help is identical for every suite, so one target is enough.
    if args.help {
        return match targets.first() {
            Some(target) => {
                run_suite_file(target, &plugin_crates, &args)?;
                Ok(0)
            }
            None => Ok(0),
        };
    }

    let succeeded = run_execution_targets(&targets, |target| {
        run_suite_file(target, &plugin_crates, &args)
    });

    Ok(if succeeded { 0 } else { 1 })
}

fn create_execution_targets(
    discovered: &[DiscoveredSuiteFile],
    requested_typename: Option<&str>,
    scope: &Path,
) -> std::result::Result<Vec<ExecutionTarget>, RutError> {
    if let Some(typename) = requested_typename {
        let suite_file = select_suite_file(discovered, typename, scope)?;
        return Ok(vec![ExecutionTarget {
            suite_file: suite_file.path.clone(),
            typename: typename.to_string(),
        }]);
    }

    Ok(discovered
        .iter()
        .flat_map(|suite_file| {
            suite_file.suites.iter().map(|suite| ExecutionTarget {
                suite_file: suite_file.path.clone(),
                typename: suite.typename.clone(),
            })
        })
        .collect())
}

/// Describes the suite to the harness so reporters can derive their own paths.
fn suite_environment(target: &ExecutionTarget, scope: Option<&Path>) -> Vec<(String, String)> {
    let source = std::fs::canonicalize(&target.suite_file)
        .unwrap_or_else(|_| target.suite_file.to_path_buf());
    let scope_root = match scope {
        Some(path) if path.is_file() => None,
        Some(path) => std::fs::canonicalize(path).ok(),
        None => std::env::current_dir().ok(),
    };
    let relative_dir = scope_root
        .as_deref()
        .and_then(|root| source.parent()?.strip_prefix(root).ok())
        .unwrap_or_else(|| Path::new(""));

    vec![
        ("RUT_SUITE_TYPENAME".to_string(), target.typename.clone()),
        (
            "RUT_SUITE_SLUG".to_string(),
            build::suite_slug(&target.typename),
        ),
        (
            "RUT_SUITE_FILE".to_string(),
            source.to_string_lossy().into_owned(),
        ),
        (
            "RUT_SUITE_REL_DIR".to_string(),
            relative_dir.to_string_lossy().into_owned(),
        ),
    ]
}

fn select_suite_file<'a>(
    discovered: &'a [DiscoveredSuiteFile],
    requested_typename: &str,
    scope: &Path,
) -> std::result::Result<&'a DiscoveredSuiteFile, RutError> {
    let mut matches = Vec::new();

    for suite_file in discovered {
        if suite_file
            .suites
            .iter()
            .any(|suite| suite.typename == requested_typename)
        {
            matches.push(suite_file);
        }
    }

    match matches.as_slice() {
        [] => Err(RutError::RequestedTypenameNotFound {
            typename: requested_typename.to_string(),
            scope: scope.to_path_buf(),
        }),
        [suite_file] => Ok(suite_file),
        _ => Err(RutError::AmbiguousTypename {
            typename: requested_typename.to_string(),
            candidates: matches
                .iter()
                .map(|suite_file| format!("  {}", suite_file.path.display()))
                .collect::<Vec<_>>()
                .join("\n"),
        }),
    }
}

fn run_suite_file(
    target: &ExecutionTarget,
    plugin_crates: &[PluginCrate],
    args: &RunArgs,
) -> Result<bool> {
    if !args.help {
        eprintln!("Found suite file: {}", target.suite_file.display());
        eprintln!("Found suite typename: {}", target.typename);
    }

    let wrapper = generate_wrapper(
        &target.suite_file,
        &target.typename,
        WrapperOptions { plugin_crates },
    );

    let status = build::run_temp_project(
        &target.suite_file,
        &target.typename,
        wrapper,
        plugin_crates,
        &args.harness_args,
        &suite_environment(target, args.path.as_deref()),
    )?;

    Ok(status.success())
}

fn run_execution_targets<F>(targets: &[ExecutionTarget], mut run_suite: F) -> bool
where
    F: FnMut(&ExecutionTarget) -> Result<bool>,
{
    let mut succeeded = true;

    for target in targets {
        match run_suite(target) {
            Ok(true) => {}
            Ok(false) => succeeded = false,
            Err(error) => {
                eprintln!(
                    "Failed to run suite {}: {error}",
                    target.suite_file.display()
                );
                succeeded = false;
            }
        }
    }

    succeeded
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn target(path: &str) -> ExecutionTarget {
        ExecutionTarget {
            suite_file: PathBuf::from(path),
            typename: "Suite".to_string(),
        }
    }

    fn environment(target: &ExecutionTarget, scope: Option<&Path>) -> Vec<(String, String)> {
        suite_environment(target, scope)
    }

    fn write_suite_file(root: &Path, relative_path: &str, typenames: &[&str]) -> PathBuf {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let content = typenames
            .iter()
            .map(|typename| format!("suite! {{ typename = {typename}; name = \"{typename}\"; }}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn lists_files_with_every_suite_name_and_typename() {
        let project = TempDir::new().unwrap();
        let first = project.path().join("a.rs");
        let second = project.path().join("nested/b.rs");
        std::fs::create_dir_all(second.parent().unwrap()).unwrap();
        std::fs::write(
            &first,
            r#"
suite! {
    typename = FirstSuite;
    name = "First";
    test_case(name = "only") { test(name = "one") {} }
}
"#,
        )
        .unwrap();
        std::fs::write(
            &second,
            r#"
suite! { typename = SecondSuite; name = "Second"; }
suite! {
    typename = ThirdSuite;
    name = "Third";
    test_case(name = "several") {
        test(name = "one") {}
        test(name = "two") {}
    }
}
"#,
        )
        .unwrap();

        let listing = discover_suites(Some(project.path().to_path_buf())).unwrap();

        assert_eq!(
            format_suite_listing(&listing),
            format!(
                "{}\n  First (FirstSuite): 1 case\n    only: 1 test\n{}\n  Second (SecondSuite): 0 cases\n  Third (ThirdSuite): 1 case\n    several: 2 tests\n",
                first.display(),
                second.display()
            )
        );
    }

    #[test]
    fn selects_a_non_first_typename_from_a_file() {
        let project = TempDir::new().unwrap();
        let suite_file =
            write_suite_file(project.path(), "multi.rs", &["FirstSuite", "SelectedSuite"]);
        let discovered = discover_suites(Some(suite_file.clone())).unwrap();

        let selected = select_suite_file(&discovered, "SelectedSuite", project.path()).unwrap();

        assert_eq!(selected.path, suite_file);
    }

    #[test]
    fn creates_a_target_for_every_suite_in_a_file() {
        let project = TempDir::new().unwrap();
        let suite_file =
            write_suite_file(project.path(), "multi.rs", &["FirstSuite", "SecondSuite"]);
        let discovered = discover_suites(Some(suite_file.clone())).unwrap();

        let targets = create_execution_targets(&discovered, None, project.path()).unwrap();

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].suite_file, suite_file);
        assert_eq!(targets[0].typename, "FirstSuite");
        assert_eq!(targets[1].suite_file, suite_file);
        assert_eq!(targets[1].typename, "SecondSuite");
    }

    #[test]
    fn reports_when_a_requested_typename_is_missing() {
        let project = TempDir::new().unwrap();
        let suite_file = write_suite_file(project.path(), "suite.rs", &["ExistingSuite"]);
        let discovered = discover_suites(Some(suite_file)).unwrap();

        let error = select_suite_file(&discovered, "MissingSuite", project.path()).unwrap_err();

        assert!(matches!(
            error,
            RutError::RequestedTypenameNotFound { ref typename, ref scope }
                if typename == "MissingSuite" && scope == project.path()
        ));
    }

    #[test]
    fn reports_ambiguous_typenames_and_allows_scoping() {
        let project = TempDir::new().unwrap();
        let first = write_suite_file(project.path(), "first.rs", &["SharedSuite"]);
        let second = write_suite_file(project.path(), "nested/second.rs", &["SharedSuite"]);
        let discovered = discover_suites(Some(project.path().to_path_buf())).unwrap();

        let error = select_suite_file(&discovered, "SharedSuite", project.path()).unwrap_err();

        match error {
            RutError::AmbiguousTypename {
                typename,
                candidates,
            } => {
                assert_eq!(typename, "SharedSuite");
                assert!(candidates.contains(first.to_str().unwrap()));
                assert!(candidates.contains(second.to_str().unwrap()));
            }
            other => panic!("expected ambiguous typename error, got {other}"),
        }

        let scoped = discover_suites(Some(second.clone())).unwrap();
        assert_eq!(
            select_suite_file(&scoped, "SharedSuite", second.parent().unwrap())
                .unwrap()
                .path,
            second
        );
    }

    #[test]
    fn describes_the_suite_through_the_environment() {
        let project = TempDir::new().unwrap();
        write_suite_file(project.path(), "nested/suite.rs", &["CalculatorSuite"]);
        let discovered = discover_suites(Some(project.path().to_path_buf())).unwrap();
        let targets = create_execution_targets(&discovered, None, project.path()).unwrap();

        let environment = environment(&targets[0], Some(project.path()))
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(environment["RUT_SUITE_TYPENAME"], "CalculatorSuite");
        assert_eq!(environment["RUT_SUITE_SLUG"], "calculator-suite");
        assert_eq!(environment["RUT_SUITE_REL_DIR"], "nested");
        assert!(environment["RUT_SUITE_FILE"].ends_with("nested/suite.rs"));
    }

    #[test]
    fn leaves_the_relative_directory_empty_for_a_suite_file_scope() {
        let project = TempDir::new().unwrap();
        let suite_file = write_suite_file(project.path(), "suite.rs", &["CalculatorSuite"]);
        let discovered = discover_suites(Some(suite_file.clone())).unwrap();
        let targets = create_execution_targets(&discovered, None, &suite_file).unwrap();

        let environment = environment(&targets[0], Some(&suite_file))
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(environment["RUT_SUITE_REL_DIR"], "");
    }

    #[test]
    fn runs_every_suite_and_aggregates_failures() {
        let targets = ["a.rs", "b.rs", "c.rs"]
            .into_iter()
            .map(target)
            .collect::<Vec<_>>();
        let mut attempted = Vec::new();

        let succeeded = run_execution_targets(&targets, |target| {
            attempted.push(target.suite_file.clone());
            match target.suite_file.to_str().unwrap() {
                "a.rs" => Ok(true),
                "b.rs" => Ok(false),
                _ => Err(RutError::RunError("runner failed".to_string()).into()),
            }
        });

        assert!(!succeeded);
        assert_eq!(
            attempted,
            targets
                .iter()
                .map(|target| target.suite_file.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn succeeds_when_every_suite_succeeds() {
        let targets = ["a.rs", "b.rs"].into_iter().map(target).collect::<Vec<_>>();

        assert!(run_execution_targets(&targets, |_| Ok(true)));
    }
}
