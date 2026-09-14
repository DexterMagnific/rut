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
use std::path::Path;

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
    let succeeded = if let Some(typename) = args.typename.as_deref() {
        let scope = args.path.as_deref().unwrap_or_else(|| Path::new("."));
        let suite_file = select_suite_file(&discovered, typename, scope)?;
        run_suite_files(std::slice::from_ref(suite_file), |suite_file| {
            run_suite_file(&suite_file.path, typename, &args)
        })
    } else {
        run_suite_files(&discovered, |suite_file| {
            run_suite_file(&suite_file.path, &suite_file.suites[0].typename, &args)
        })
    };

    Ok(if succeeded { 0 } else { 1 })
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

fn run_suite_file(suite_file: &Path, typename: &str, args: &RunArgs) -> Result<bool> {
    eprintln!("Found suite file: {}", suite_file.display());
    eprintln!("Found suite typename: {}", typename);

    let wrapper = generate_wrapper(
        suite_file,
        typename,
        args.runner.clone(),
        args.jobs,
        args.shuffle,
    );

    let status = build::run_temp_project(suite_file, typename, wrapper)?;

    Ok(status.success())
}

fn run_suite_files<F>(suite_files: &[DiscoveredSuiteFile], mut run_suite: F) -> bool
where
    F: FnMut(&DiscoveredSuiteFile) -> Result<bool>,
{
    let mut succeeded = true;

    for suite_file in suite_files {
        match run_suite(suite_file) {
            Ok(true) => {}
            Ok(false) => succeeded = false,
            Err(error) => {
                eprintln!("Failed to run suite {}: {error}", suite_file.path.display());
                succeeded = false;
            }
        }
    }

    succeeded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

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
    fn runs_every_suite_and_aggregates_failures() {
        let suite_files = ["a.rs", "b.rs", "c.rs"]
            .into_iter()
            .map(|path| DiscoveredSuiteFile {
                path: PathBuf::from(path),
                suites: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut attempted = Vec::new();

        let succeeded = run_suite_files(&suite_files, |suite_file| {
            attempted.push(suite_file.path.clone());
            match suite_file.path.to_str().unwrap() {
                "a.rs" => Ok(true),
                "b.rs" => Ok(false),
                _ => Err(RutError::RunError("runner failed".to_string()).into()),
            }
        });

        assert!(!succeeded);
        assert_eq!(
            attempted,
            suite_files
                .iter()
                .map(|suite_file| suite_file.path.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn succeeds_when_every_suite_succeeds() {
        let suite_files = ["a.rs", "b.rs"]
            .into_iter()
            .map(|path| DiscoveredSuiteFile {
                path: PathBuf::from(path),
                suites: Vec::new(),
            })
            .collect::<Vec<_>>();

        assert!(run_suite_files(&suite_files, |_| Ok(true)));
    }
}
