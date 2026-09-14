mod build;
mod cli;
mod discovery;
mod error;
mod parser;
mod wrapper;

use crate::cli::{Cli, Commands, ListArgs, RunArgs};
use crate::discovery::{DiscoveredSuiteFile, discover_suites};
use crate::error::{Result, RutError};
use crate::wrapper::generate_wrapper;
use clap::Parser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct ExecutionTarget {
    suite_file: PathBuf,
    typename: String,
    junit_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(&args[1..]);

    let exit_code = match cli.command {
        Commands::Run(args) => run_command(args)?,
        Commands::List(args) => list_command(args)?,
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
    let discovered = discover_suites(args.path.clone())?;
    let scope = args.path.as_deref().unwrap_or_else(|| Path::new("."));
    let mut targets = create_execution_targets(&discovered, args.typename.as_deref(), scope)?;
    resolve_junit_outputs(&mut targets, &args)?;
    let succeeded = run_execution_targets(&targets, |target| {
        run_suite_file(
            &target.suite_file,
            &target.typename,
            target.junit_path.as_deref(),
            &args,
        )
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
            junit_path: None,
        }]);
    }

    Ok(discovered
        .iter()
        .map(|suite_file| ExecutionTarget {
            suite_file: suite_file.path.clone(),
            typename: suite_file.suites[0].typename.clone(),
            junit_path: None,
        })
        .collect())
}

fn resolve_junit_outputs(targets: &mut [ExecutionTarget], args: &RunArgs) -> Result<()> {
    if let Some(path) = &args.junit {
        if targets.len() != 1 {
            return Err(RutError::JUnitOutput(format!(
                "--junit requires exactly one selected suite, but {} were selected; use --junit-dir for multiple suites",
                targets.len()
            ))
            .into());
        }
        let path = absolute_path(path)?;
        create_report_parent(&path)?;
        targets[0].junit_path = Some(path);
    } else if let Some(directory) = &args.junit_dir {
        let directory = absolute_path(directory)?;
        let scope_root = match args.path.as_deref() {
            Some(path) if path.is_file() => None,
            Some(path) => Some(std::fs::canonicalize(path).map_err(|error| {
                RutError::JUnitOutput(format!(
                    "failed to resolve report source scope {}: {error}",
                    path.display()
                ))
            })?),
            None => Some(std::env::current_dir().map_err(|error| {
                RutError::JUnitOutput(format!("failed to resolve current directory: {error}"))
            })?),
        };

        let mut destinations = HashMap::<PathBuf, Vec<String>>::new();
        for target in targets.iter_mut() {
            let source = std::fs::canonicalize(&target.suite_file).map_err(|error| {
                RutError::JUnitOutput(format!(
                    "failed to resolve suite source {}: {error}",
                    target.suite_file.display()
                ))
            })?;
            let relative_parent = scope_root
                .as_deref()
                .and_then(|root| source.parent()?.strip_prefix(root).ok())
                .unwrap_or_else(|| Path::new(""));
            let destination = directory
                .join(relative_parent)
                .join(format!("{}.xml", build::suite_slug(&target.typename)));
            destinations
                .entry(destination.clone())
                .or_default()
                .push(format!(
                    "{} ({})",
                    target.suite_file.display(),
                    target.typename
                ));
            target.junit_path = Some(destination);
        }

        if let Some((path, suites)) = destinations.iter().find(|(_, suites)| suites.len() > 1) {
            return Err(RutError::JUnitOutput(format!(
                "multiple suites map to {}:\n  {}",
                path.display(),
                suites.join("\n  ")
            ))
            .into());
        }

        for path in destinations.keys() {
            create_report_parent(path)?;
        }
    }

    Ok(())
}

fn absolute_path(path: &Path) -> std::result::Result<PathBuf, RutError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| {
                RutError::JUnitOutput(format!("failed to resolve current directory: {error}"))
            })
    }
}

fn create_report_parent(path: &Path) -> std::result::Result<(), RutError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            RutError::JUnitOutput(format!(
                "failed to create report directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    Ok(())
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
    suite_file: &Path,
    typename: &str,
    junit_path: Option<&Path>,
    args: &RunArgs,
) -> Result<bool> {
    eprintln!("Found suite file: {}", suite_file.display());
    eprintln!("Found suite typename: {}", typename);

    let wrapper = generate_wrapper(
        suite_file,
        typename,
        args.runner.clone(),
        args.jobs,
        args.shuffle,
        junit_path,
    );

    let status = build::run_temp_project(suite_file, typename, wrapper)?;

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
    use crate::cli::RunnerType;
    use tempfile::TempDir;

    fn run_args(path: Option<PathBuf>) -> RunArgs {
        RunArgs {
            path,
            typename: None,
            runner: RunnerType::Parallel,
            jobs: None,
            shuffle: false,
            junit: None,
            junit_dir: None,
        }
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
    fn rejects_a_single_junit_file_for_multiple_targets() {
        let mut targets = ["a.rs", "b.rs"]
            .into_iter()
            .map(|path| ExecutionTarget {
                suite_file: PathBuf::from(path),
                typename: "Suite".to_string(),
                junit_path: None,
            })
            .collect::<Vec<_>>();
        let mut args = run_args(None);
        args.junit = Some(PathBuf::from("report.xml"));

        let error = resolve_junit_outputs(&mut targets, &args).unwrap_err();

        assert!(error.to_string().contains("--junit-dir"));
    }

    #[test]
    fn mirrors_source_directories_with_predictable_junit_names() {
        let project = TempDir::new().unwrap();
        let first = write_suite_file(project.path(), "first.rs", &["FirstSuite"]);
        let second = write_suite_file(project.path(), "nested/second.rs", &["SecondSuite"]);
        let discovered = discover_suites(Some(project.path().to_path_buf())).unwrap();
        let mut targets = create_execution_targets(&discovered, None, project.path()).unwrap();
        let report_dir = project.path().join("reports");
        let mut args = run_args(Some(project.path().to_path_buf()));
        args.junit_dir = Some(report_dir.clone());

        resolve_junit_outputs(&mut targets, &args).unwrap();

        let destinations = targets
            .iter()
            .map(|target| target.junit_path.as_ref().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(targets[0].suite_file, first);
        assert_eq!(targets[1].suite_file, second);
        assert_eq!(destinations[0], &report_dir.join("first-suite.xml"));
        assert_eq!(destinations[1], &report_dir.join("nested/second-suite.xml"));
    }

    #[test]
    fn rejects_same_directory_junit_slug_collisions() {
        let project = TempDir::new().unwrap();
        write_suite_file(project.path(), "first.rs", &["SharedSuite"]);
        write_suite_file(project.path(), "second.rs", &["SharedSuite"]);
        let discovered = discover_suites(Some(project.path().to_path_buf())).unwrap();
        let mut targets = create_execution_targets(&discovered, None, project.path()).unwrap();
        let mut args = run_args(Some(project.path().to_path_buf()));
        args.junit_dir = Some(project.path().join("reports"));

        let error = resolve_junit_outputs(&mut targets, &args).unwrap_err();

        assert!(error.to_string().contains("shared-suite.xml"));
        assert!(error.to_string().contains("first.rs"));
        assert!(error.to_string().contains("second.rs"));
    }

    #[test]
    fn runs_every_suite_and_aggregates_failures() {
        let targets = ["a.rs", "b.rs", "c.rs"]
            .into_iter()
            .map(|path| ExecutionTarget {
                suite_file: PathBuf::from(path),
                typename: "Suite".to_string(),
                junit_path: None,
            })
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
        let targets = ["a.rs", "b.rs"]
            .into_iter()
            .map(|path| ExecutionTarget {
                suite_file: PathBuf::from(path),
                typename: "Suite".to_string(),
                junit_path: None,
            })
            .collect::<Vec<_>>();

        assert!(run_execution_targets(&targets, |_| Ok(true)));
    }
}
