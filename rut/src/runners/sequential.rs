use crate::report::{BoxFuture, SuiteReport};
use std::time::{Duration, Instant};

pub struct SequentialRunner {
    suite: Option<Box<dyn crate::suite::TestSuiteInternal>>,
    reporter: Option<Box<dyn crate::reporter::TestReporterInternal>>,
}

impl SequentialRunner {
    pub fn new() -> Self {
        Self {
            suite: None,
            reporter: None,
        }
    }
}

impl Default for SequentialRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::runner::TestRunnerInternal for SequentialRunner {
    fn with_suite(mut self, suite: Box<dyn crate::suite::TestSuiteInternal>) -> Self {
        self.suite = Some(suite);
        self
    }

    fn with_reporter(mut self, reporter: Box<dyn crate::reporter::TestReporterInternal>) -> Self {
        self.reporter = Some(reporter);
        self
    }

    fn run(self) -> BoxFuture<'static, SuiteReport> {
        Box::pin(async move {
            let suite = self.suite.expect("suite required");
            let mut reporter = self
                .reporter
                .unwrap_or_else(|| Box::new(crate::reporters::StdoutReporter::new()));
            let suite_name = suite.name().to_owned();

            let test_cases = suite.test_cases();
            let test_case_infos: Vec<crate::report::TestCaseInfo> = test_cases
                .iter()
                .map(|c| crate::report::TestCaseInfo {
                    name: c.name().to_string(),
                    test_names: c.tests().iter().map(|t| t.name().to_string()).collect(),
                })
                .collect::<Vec<_>>();

            reporter.report_start(&suite_name, &test_case_infos).await;

            // Run setup with panic catching
            let suite_total_start = Instant::now();
            let setup_result = tokio::task::spawn(async move {
                let mut suite = suite;
                suite.setup_suite().await;
                suite
            })
            .await;

            let suite = match setup_result {
                Ok(suite) => suite,
                Err(_) => {
                    reporter
                        .report_finish(Duration::ZERO, suite_total_start.elapsed())
                        .await;
                    return reporter.get_report().clone();
                }
            };

            let ctx = suite.context();
            let suite_duration_start = Instant::now();

            for case in test_cases {
                let mut case = case.clone_box();
                reporter
                    .report_case_start(case.name(), case.tests().len())
                    .await;

                let case_total_start = Instant::now();
                case.setup_case(ctx).await;
                let case_duration_start = Instant::now();

                for test in case.tests() {
                    reporter.report_test_start(case.name(), test.name()).await;

                    let test_start = Instant::now();
                    let mut result = test.run(ctx).await;
                    let duration = test_start.elapsed();

                    result.duration = duration;
                    result.total_duration = duration;

                    reporter.report_result(case.name(), &result).await;

                    if result.status == crate::report::TestStatus::Failed {
                        break;
                    }
                }

                let case_duration = case_duration_start.elapsed();
                case.teardown_case(ctx).await;
                let case_total_duration = case_total_start.elapsed();
                reporter
                    .report_case_finish(case.name(), case_duration, case_total_duration)
                    .await;
            }

            // Run teardown with panic catching
            let suite_duration = suite_duration_start.elapsed();
            let _ = tokio::task::spawn(async move {
                let mut suite = suite;
                suite.teardown_suite().await
            })
            .await;

            let suite_total_duration = suite_total_start.elapsed();
            reporter
                .report_finish(suite_duration, suite_total_duration)
                .await;
            reporter.get_report().clone()
        })
    }
}
