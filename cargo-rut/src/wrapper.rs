use crate::cli::RunnerType;
use std::path::Path;

pub struct WrapperOptions<'a> {
    pub runner: RunnerType,
    pub jobs: Option<usize>,
    pub shuffle: bool,
    pub filters: &'a [String],
    pub fail_fast: bool,
    pub junit_path: Option<&'a Path>,
    pub gtest_path: Option<&'a Path>,
}

pub fn generate_wrapper(suite_file: &Path, typename: &str, options: WrapperOptions<'_>) -> String {
    // Convert to absolute path so include! can find it from temp directory
    let absolute_suite_path = std::fs::canonicalize(suite_file)
        .unwrap_or_else(|_| suite_file.to_path_buf())
        .display()
        .to_string();

    let reporter_code = if options.junit_path.is_none() && options.gtest_path.is_none() {
        "rut::StdoutReporter::new()".to_string()
    } else {
        let mut code = "rut::MultiReporter::new()\n                    .add_reporter(Box::new(rut::StdoutReporter::new()))".to_string();
        if let Some(path) = options.junit_path {
            let path_literal = format!("{:?}", path.to_string_lossy());
            code.push_str(&format!(
                "\n                    .add_reporter(Box::new(rut::JUnitReporter::new({path_literal})))"
            ));
        }
        if let Some(path) = options.gtest_path {
            let path_literal = format!("{:?}", path.to_string_lossy());
            code.push_str(&format!(
                "\n                    .add_reporter(Box::new(rut::GTestReporter::new({path_literal})))"
            ));
        }
        code
    };

    let filter_code = options
        .filters
        .iter()
        .map(|filter| format!(".with_filter({filter:?})"))
        .collect::<String>();
    let fail_fast_code = if options.fail_fast {
        ".fail_fast()"
    } else {
        ""
    };

    let runner_code = match options.runner {
        RunnerType::Parallel => {
            let jobs_code = options
                .jobs
                .map(|j| format!(".with_max_jobs({})", j))
                .unwrap_or_default();
            let shuffle_code = if options.shuffle {
                ".shuffle_test_cases()"
            } else {
                ""
            };
            format!(
                r#"rut::ParallelRunnerBuilder::new(){}{}{}{}
                    .with_suite(Box::new({}::new()))
                    .with_reporter(Box::new({}))
                    .build()"#,
                jobs_code, shuffle_code, filter_code, fail_fast_code, typename, reporter_code
            )
        }
        RunnerType::Sequential => format!(
            r#"rut::SequentialRunner::new(){}{}
                .with_suite(Box::new({}::new()))
                .with_reporter(Box::new({}))"#,
            filter_code, fail_fast_code, typename, reporter_code
        ),
    };

    format!(
        r#"include!(r"{}");

use rut::TestRunner as _;

#[tokio::main]
async fn main() {{
    let exit_code = match {}
        .run()
        .await
    {{
        Ok(report) => if report.total_failed > 0 {{ 1 }} else {{ 0 }},
        Err(error) => {{
            eprintln!("Reporting failed: {{error}}");
            2
        }}
    }};
    std::process::exit(exit_code);
}}"#,
        absolute_suite_path, runner_code
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn options(runner: RunnerType) -> WrapperOptions<'static> {
        WrapperOptions {
            runner,
            jobs: None,
            shuffle: false,
            filters: &[],
            fail_fast: false,
            junit_path: None,
            gtest_path: None,
        }
    }

    #[test]
    fn test_generate_wrapper_parallel_default() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            options(RunnerType::Parallel),
        );
        assert!(wrapper.contains("ParallelRunnerBuilder::new()"));
        assert!(wrapper.contains("use rut::TestRunner as _;"));
        assert!(!wrapper.contains("TestRunnerInternal"));
        assert!(wrapper.contains("CalculatorSuite::new()"));
        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(!wrapper.contains("with_max_jobs"));
        assert!(!wrapper.contains("shuffle_test_cases"));
    }

    #[test]
    fn test_generate_wrapper_parallel_with_jobs_and_shuffle() {
        let mut options = options(RunnerType::Parallel);
        options.jobs = Some(4);
        options.shuffle = true;
        let wrapper = generate_wrapper(&PathBuf::from("test_suite.rs"), "CalculatorSuite", options);
        assert!(wrapper.contains("with_max_jobs(4)"));
        assert!(wrapper.contains("shuffle_test_cases()"));
    }

    #[test]
    fn test_generate_wrapper_sequential() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            options(RunnerType::Sequential),
        );
        assert!(wrapper.contains("SequentialRunner::new()"));
        assert!(wrapper.contains("CalculatorSuite::new()"));
        assert!(!wrapper.contains("ParallelRunnerBuilder"));
    }

    #[test]
    fn test_generate_wrapper_with_junit_reporter() {
        let mut options = options(RunnerType::Sequential);
        options.junit_path = Some(Path::new("reports/a report.xml"));
        let wrapper = generate_wrapper(&PathBuf::from("test_suite.rs"), "CalculatorSuite", options);

        assert!(wrapper.contains("MultiReporter::new()"));
        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(wrapper.contains("JUnitReporter::new(\"reports/a report.xml\")"));
        assert!(wrapper.contains("Reporting failed"));
    }

    #[test]
    fn test_generate_wrapper_with_junit_and_gtest_reporters() {
        let mut options = options(RunnerType::Sequential);
        options.junit_path = Some(Path::new("reports/a report.xml"));
        options.gtest_path = Some(Path::new("reports/a report.json"));
        let wrapper = generate_wrapper(&PathBuf::from("test_suite.rs"), "CalculatorSuite", options);

        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(wrapper.contains("JUnitReporter::new(\"reports/a report.xml\")"));
        assert!(wrapper.contains("GTestReporter::new(\"reports/a report.json\")"));
    }

    #[test]
    fn test_generate_wrapper_with_filters() {
        let filters = ["addition".to_string(), "edge \"case\"".to_string()];
        let mut options = options(RunnerType::Sequential);
        options.filters = &filters;
        options.fail_fast = true;
        let wrapper = generate_wrapper(&PathBuf::from("test_suite.rs"), "CalculatorSuite", options);

        assert!(wrapper.contains(".with_filter(\"addition\")"));
        assert!(wrapper.contains(".with_filter(\"edge \\\"case\\\"\")"));
        assert!(wrapper.contains(".fail_fast()"));
    }
}
