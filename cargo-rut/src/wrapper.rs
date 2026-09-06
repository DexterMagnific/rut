use crate::cli::RunnerType;
use std::path::Path;

pub fn generate_wrapper(
    suite_file: &Path,
    typename: &str,
    runner: RunnerType,
    jobs: Option<usize>,
    shuffle: bool,
) -> String {
    // Convert to absolute path so include! can find it from temp directory
    let absolute_suite_path = std::fs::canonicalize(suite_file)
        .unwrap_or_else(|_| suite_file.to_path_buf())
        .display()
        .to_string();

    let runner_code = match runner {
        RunnerType::Parallel => {
            let jobs_code = jobs
                .map(|j| format!(".with_max_jobs({})", j))
                .unwrap_or_default();
            let shuffle_code = if shuffle {
                ".shuffle_test_cases()"
            } else {
                ""
            };
            format!(
                r#"rut::ParallelRunnerBuilder::new(){}{}
                    .with_suite(Box::new({}::new()))
                    .with_reporter(Box::new(rut::StdoutReporter::new()))
                    .build()"#,
                jobs_code, shuffle_code, typename
            )
        }
        RunnerType::Sequential => format!(
            r#"rut::SequentialRunner::new()
                .with_suite(Box::new({}::new()))
                .with_reporter(Box::new(rut::StdoutReporter::new()))"#,
            typename
        ),
    };

    format!(
        r#"include!(r"{}");

#[tokio::main]
async fn main() {{
    let report = {}
        .run()
        .await;
    std::process::exit(if report.total_failed > 0 {{ 1 }} else {{ 0 }});
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
        );
        assert!(wrapper.contains("SequentialRunner::new()"));
        assert!(wrapper.contains("CalculatorSuite::new()"));
        assert!(!wrapper.contains("ParallelRunnerBuilder"));
    }
}