use crate::report::SuiteReport;
use crate::reporter::{ReporterResult, TestReporter};
use crate::suite::TestSuite;
use async_trait::async_trait;
use chrono::Utc;
use std::time::{Duration, Instant};

use super::filter::{SelectedCase, select_cases};

pub struct SequentialRunner {
    suite: Option<Box<dyn TestSuite>>,
    reporter: Option<Box<dyn TestReporter>>,
    filters: Vec<String>,
    fail_fast: bool,
}

impl SequentialRunner {
    pub fn new() -> Self {
        Self {
            suite: None,
            reporter: None,
            filters: Vec::new(),
            fail_fast: false,
        }
    }

    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filters.push(filter.into());
        self
    }

    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }
}

impl Default for SequentialRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::runner::TestRunner for SequentialRunner {
    fn with_suite(mut self, suite: Box<dyn TestSuite>) -> Self {
        self.suite = Some(suite);
        self
    }

    fn with_reporter(mut self, reporter: Box<dyn TestReporter>) -> Self {
        self.reporter = Some(reporter);
        self
    }

    async fn run(self) -> ReporterResult<SuiteReport> {
        let fail_fast = self.fail_fast;
        let suite = self.suite.expect("suite required");
        let mut reporter = self
            .reporter
            .unwrap_or_else(|| Box::new(crate::reporters::StdoutReporter::new()));
        let suite_name = suite.name().to_owned();

        let selected_cases = select_cases(&suite_name, suite.test_cases(), &self.filters);
        let test_case_infos: Vec<crate::report::TestCaseInfo> = selected_cases
            .iter()
            .map(|case| crate::report::TestCaseInfo {
                name: case.name.clone(),
                tests: case
                    .tests
                    .iter()
                    .map(|test| crate::report::TestInfo {
                        name: test.name().to_string(),
                        source: test.source_location(),
                    })
                    .collect(),
            })
            .collect();

        let suite_started_at = Utc::now();
        reporter
            .report_start(&suite_name, &test_case_infos, suite_started_at)
            .await?;

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
                    .report_finish(Duration::ZERO, suite_total_start.elapsed(), Utc::now())
                    .await?;
                return Ok(reporter.get_report().clone());
            }
        };

        let ctx = suite.context();
        let suite_duration_start = Instant::now();

        for SelectedCase {
            name: case_name,
            mut case,
            tests,
        } in selected_cases
        {
            let mut case_failed = false;
            let case_started_at = Utc::now();
            reporter
                .report_case_start(&case_name, tests.len(), case_started_at)
                .await?;

            let case_total_start = Instant::now();
            case.setup_case(ctx).await;
            let case_duration_start = Instant::now();

            for test in tests {
                reporter.report_test_start(&case_name, test.name()).await?;

                let test_start = Instant::now();
                let mut result = match crate::panic_capture::catch_test_panic(test.run(ctx)).await {
                    Ok(result) | Err(result) => result,
                };
                let duration = test_start.elapsed();

                if result.name.is_empty() {
                    result.name = test.name().to_string();
                }
                result.source = test.source_location();
                result.duration = duration;
                result.total_duration = duration;

                reporter.report_result(&case_name, &result).await?;

                if result.status == crate::report::TestStatus::Failed {
                    case_failed = true;
                    break;
                }
            }

            let case_duration = case_duration_start.elapsed();
            case.teardown_case(ctx).await;
            let case_total_duration = case_total_start.elapsed();
            reporter
                .report_case_finish(&case_name, case_duration, case_total_duration, Utc::now())
                .await?;

            if fail_fast && case_failed {
                break;
            }
        }

        let suite_duration = suite_duration_start.elapsed();
        let _ = tokio::task::spawn(async move {
            let mut suite = suite;
            suite.teardown_suite().await
        })
        .await;

        let suite_total_duration = suite_total_start.elapsed();
        reporter
            .report_finish(suite_duration, suite_total_duration, Utc::now())
            .await?;
        Ok(reporter.get_report().clone())
    }
}
