use crate::report::SuiteReport;
use crate::reporter::{ReporterResult, TestReporter};
use crate::suite::TestSuite;
use async_trait::async_trait;
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

use crate::report::TestResult;

use super::filter::{SelectedCase, select_cases};

struct CompletedCase {
    name: String,
    test_count: usize,
    results: Vec<TestResult>,
    started_at: chrono::DateTime<chrono::Utc>,
    finished_at: chrono::DateTime<chrono::Utc>,
    duration: Duration,
    total_duration: Duration,
}

pub struct ParallelRunner {
    max_jobs: usize,
    shuffle_test_cases: bool,
    suite: Option<Box<dyn TestSuite>>,
    reporter: Option<Box<dyn TestReporter>>,
    filters: Vec<String>,
    fail_fast: bool,
}

impl ParallelRunner {
    /// Creates a parallel runner with an explicit case concurrency limit.
    pub fn new(max_jobs: usize) -> Self {
        Self {
            max_jobs,
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
            filters: Vec::new(),
            fail_fast: false,
        }
    }

    /// Creates a parallel runner using the available CPU count as its limit.
    pub fn new_default() -> Self {
        Self {
            max_jobs: num_cpus::get(),
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
            filters: Vec::new(),
            fail_fast: false,
        }
    }

    /// Adds a qualified-name substring filter.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filters.push(filter.into());
        self
    }

    /// Stops dispatching new cases after the first failed case.
    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }
}

impl Default for ParallelRunner {
    fn default() -> Self {
        Self::new_default()
    }
}

pub struct ParallelRunnerBuilder {
    max_jobs: usize,
    shuffle_test_cases: bool,
    suite: Option<Box<dyn TestSuite>>,
    reporter: Option<Box<dyn TestReporter>>,
    filters: Vec<String>,
    fail_fast: bool,
}

impl ParallelRunnerBuilder {
    /// Creates a builder using the available CPU count as its job limit.
    pub fn new() -> Self {
        Self {
            max_jobs: num_cpus::get(),
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
            filters: Vec::new(),
            fail_fast: false,
        }
    }

    /// Sets the maximum number of cases that may run concurrently.
    pub fn with_max_jobs(mut self, n: usize) -> Self {
        self.max_jobs = n;
        self
    }

    /// Randomizes case dispatch order while preserving declaration order in reports.
    pub fn shuffle_test_cases(mut self) -> Self {
        self.shuffle_test_cases = true;
        self
    }

    /// Adds a qualified-name substring filter.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filters.push(filter.into());
        self
    }

    /// Stops dispatching new cases after the first failed case.
    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }

    /// Supplies the suite to execute.
    pub fn with_suite(mut self, suite: Box<dyn TestSuite>) -> Self {
        self.suite = Some(suite);
        self
    }

    /// Supplies the reporter that receives execution events.
    pub fn with_reporter(mut self, reporter: Box<dyn TestReporter>) -> Self {
        self.reporter = Some(reporter);
        self
    }

    /// Builds the configured parallel runner.
    pub fn build(self) -> ParallelRunner {
        ParallelRunner {
            max_jobs: self.max_jobs,
            shuffle_test_cases: self.shuffle_test_cases,
            suite: self.suite,
            reporter: self.reporter,
            filters: self.filters,
            fail_fast: self.fail_fast,
        }
    }
}

impl Default for ParallelRunnerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::runner::TestRunner for ParallelRunner {
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
            .collect::<Vec<_>>();

        let suite_started_at = chrono::Utc::now();
        reporter
            .report_start(&suite_name, &test_case_infos, suite_started_at)
            .await?;

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
                    .report_finish(
                        Duration::ZERO,
                        suite_total_start.elapsed(),
                        chrono::Utc::now(),
                    )
                    .await?;
                return Ok(reporter.get_report().clone());
            }
        };

        let ctx = suite.context().cloned();
        let suite_duration_start = Instant::now();

        let mut queued: VecDeque<SelectedCase> = selected_cases.into();

        if self.shuffle_test_cases {
            shuffle_queue(queued.make_contiguous(), &mut rand::rng());
        }

        let max_jobs = self.max_jobs;
        let mut running = JoinSet::new();
        let mut failure_observed = false;

        // Start initial batch
        for _ in 0..max_jobs.min(queued.len()) {
            let ctx = ctx.clone();
            running.spawn(run_test_case(
                queued.pop_front().expect("queued case should exist"),
                ctx,
            ));
        }

        // Process completions, start next in queue order
        while let Some(res) = running.join_next().await {
            match res {
                Ok(Ok(payload)) => {
                    let case_failed = payload.results.iter().any(|result| {
                        matches!(
                            result.status,
                            crate::report::TestStatus::Failed | crate::report::TestStatus::TimedOut
                        )
                    });
                    reporter
                        .report_case_start(&payload.name, payload.test_count, payload.started_at)
                        .await?;

                    for result in payload.results {
                        reporter
                            .report_test_start(&payload.name, &result.name)
                            .await?;
                        reporter.report_result(&payload.name, &result).await?;

                        if matches!(
                            result.status,
                            crate::report::TestStatus::Failed | crate::report::TestStatus::TimedOut
                        ) {
                            break;
                        }
                    }

                    reporter
                        .report_case_finish(
                            &payload.name,
                            payload.duration,
                            payload.total_duration,
                            payload.finished_at,
                        )
                        .await?;
                    failure_observed |= case_failed;
                }
                Ok(Err(error_msg)) => {
                    failure_observed = true;
                    let started_at = chrono::Utc::now();
                    reporter.report_case_start("unknown", 0, started_at).await?;
                    reporter
                        .report_result(
                            "unknown",
                            &TestResult::failed(format!("test case panicked: {}", error_msg)),
                        )
                        .await?;
                    reporter
                        .report_case_finish(
                            "unknown",
                            Duration::ZERO,
                            Duration::ZERO,
                            chrono::Utc::now(),
                        )
                        .await?;
                }
                Err(join_error) => {
                    failure_observed = true;
                    let join_error: tokio::task::JoinError = join_error;
                    let started_at = chrono::Utc::now();
                    reporter.report_case_start("unknown", 0, started_at).await?;
                    reporter
                        .report_result(
                            "unknown",
                            &TestResult::failed(format!("test case panicked: {}", join_error)),
                        )
                        .await?;
                    reporter
                        .report_case_finish(
                            "unknown",
                            Duration::ZERO,
                            Duration::ZERO,
                            chrono::Utc::now(),
                        )
                        .await?;
                }
            }

            if (!fail_fast || !failure_observed)
                && let Some(case) = queued.pop_front()
            {
                let ctx = ctx.clone();
                running.spawn(run_test_case(case, ctx));
            }
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
            .report_finish(suite_duration, suite_total_duration, chrono::Utc::now())
            .await?;
        Ok(reporter.get_report().clone())
    }
}

fn shuffle_queue<T, R: Rng + ?Sized>(queue: &mut [T], rng: &mut R) {
    queue.shuffle(rng);
}

async fn run_test_case(
    selected: SelectedCase,
    ctx: Option<crate::report::TestContext>,
) -> Result<CompletedCase, String> {
    let SelectedCase {
        name: case_name,
        mut case,
        tests,
    } = selected;
    let test_count = tests.len();
    let started_at = chrono::Utc::now();
    let case_total_start = Instant::now();
    case.setup_case(ctx.as_ref()).await;
    let case_duration_start = Instant::now();

    let mut results = Vec::new();

    for test in tests {
        let test_start = Instant::now();
        let mut result =
            crate::runner::run_test_with_retries(test.as_ref(), ctx.as_ref()).await;
        let duration = test_start.elapsed();

        if result.name.is_empty() {
            result.name = test.name().to_string();
        }
        result.source = test.source_location();
        result.duration = duration;
        result.total_duration = duration;

        results.push(result);

        if matches!(
            results.last().unwrap().status,
            crate::report::TestStatus::Failed | crate::report::TestStatus::TimedOut
        ) {
            break;
        }
    }

    let duration = case_duration_start.elapsed();
    case.teardown_case(ctx.as_ref()).await;
    let finished_at = chrono::Utc::now();

    Ok(CompletedCase {
        name: case_name,
        test_count,
        results,
        started_at,
        finished_at,
        duration,
        total_duration: case_total_start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn builder_disables_shuffling_by_default() {
        let runner = ParallelRunnerBuilder::new().build();

        assert!(!runner.shuffle_test_cases);
    }

    #[test]
    fn builder_enables_shuffling() {
        let runner = ParallelRunnerBuilder::new().shuffle_test_cases().build();

        assert!(runner.shuffle_test_cases);
    }

    #[test]
    fn shuffle_queue_randomizes_without_losing_items() {
        let mut queue = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let original = queue.clone();
        let mut rng = StdRng::seed_from_u64(42);

        shuffle_queue(&mut queue, &mut rng);

        assert_ne!(queue, original);
        queue.sort_unstable();
        assert_eq!(queue, original);
    }
}
