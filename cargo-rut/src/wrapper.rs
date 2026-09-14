use crate::cli::RunnerType;
use std::path::Path;

pub fn generate_wrapper(
    suite_file: &Path,
    typename: &str,
    runner: RunnerType,
    jobs: Option<usize>,
    shuffle: bool,
    junit_path: Option<&Path>,
    gtest_path: Option<&Path>,
) -> String {
    // Convert to absolute path so include! can find it from temp directory
    let absolute_suite_path = std::fs::canonicalize(suite_file)
        .unwrap_or_else(|_| suite_file.to_path_buf())
        .display()
        .to_string();

    let reporter_code = if junit_path.is_none() && gtest_path.is_none() {
        "rut::StdoutReporter::new()".to_string()
    } else {
        let mut code = "rut::MultiReporter::new()\n                    .add_reporter(Box::new(rut::StdoutReporter::new()))".to_string();
        if let Some(path) = junit_path {
            let path_literal = format!("{:?}", path.to_string_lossy());
            code.push_str(&format!(
                "\n                    .add_reporter(Box::new(rut::JUnitReporter::new({path_literal})))"
            ));
        }
        if let Some(path) = gtest_path {
            let path_literal = format!("{:?}", path.to_string_lossy());
            code.push_str(&format!(
                "\n                    .add_reporter(Box::new(rut::GTestReporter::new({path_literal})))"
            ));
        }
        code
    };

    let runner_code = match runner {
        RunnerType::Parallel => {
            let jobs_code = jobs
                .map(|j| format!(".with_max_jobs({})", j))
                .unwrap_or_default();
            let shuffle_code = if shuffle { ".shuffle_test_cases()" } else { "" };
            format!(
                r#"rut::ParallelRunnerBuilder::new(){}{}
                    .with_suite(Box::new({}::new()))
                    .with_reporter(Box::new({}))
                    .build()"#,
                jobs_code, shuffle_code, typename, reporter_code
            )
        }
        RunnerType::Sequential => format!(
            r#"rut::SequentialRunner::new()
                .with_suite(Box::new({}::new()))
                .with_reporter(Box::new({}))"#,
            typename, reporter_code
        ),
    };

    format!(
        r#"include!(r"{}");

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

    #[test]
    fn test_generate_wrapper_parallel_default() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            RunnerType::Parallel,
            None,
            false,
            None,
            None,
        );
        assert!(wrapper.contains("ParallelRunnerBuilder::new()"));
        assert!(wrapper.contains("CalculatorSuite::new()"));
        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(!wrapper.contains("with_max_jobs"));
        assert!(!wrapper.contains("shuffle_test_cases"));
    }

    #[test]
    fn test_generate_wrapper_parallel_with_jobs_and_shuffle() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            RunnerType::Parallel,
            Some(4),
            true,
            None,
            None,
        );
        assert!(wrapper.contains("with_max_jobs(4)"));
        assert!(wrapper.contains("shuffle_test_cases()"));
    }

    #[test]
    fn test_generate_wrapper_sequential() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            RunnerType::Sequential,
            None,
            false,
            None,
            None,
        );
        assert!(wrapper.contains("SequentialRunner::new()"));
        assert!(wrapper.contains("CalculatorSuite::new()"));
        assert!(!wrapper.contains("ParallelRunnerBuilder"));
    }

    #[test]
    fn test_generate_wrapper_with_junit_reporter() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            RunnerType::Sequential,
            None,
            false,
            Some(Path::new("reports/a report.xml")),
            None,
        );

        assert!(wrapper.contains("MultiReporter::new()"));
        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(wrapper.contains("JUnitReporter::new(\"reports/a report.xml\")"));
        assert!(wrapper.contains("Reporting failed"));
    }

    #[test]
    fn test_generate_wrapper_with_junit_and_gtest_reporters() {
        let wrapper = generate_wrapper(
            &PathBuf::from("test_suite.rs"),
            "CalculatorSuite",
            RunnerType::Sequential,
            None,
            false,
            Some(Path::new("reports/a report.xml")),
            Some(Path::new("reports/a report.json")),
        );

        assert!(wrapper.contains("StdoutReporter::new()"));
        assert!(wrapper.contains("JUnitReporter::new(\"reports/a report.xml\")"));
        assert!(wrapper.contains("GTestReporter::new(\"reports/a report.json\")"));
    }
}
