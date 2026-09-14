use crate::report::{BoxFuture, SuiteReport};
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

use crate::report::TestResult;
use crate::suite::TestSuiteInternal;

pub struct ParallelRunner {
    max_jobs: usize,
    shuffle_test_cases: bool,
    suite: Option<Box<dyn TestSuiteInternal>>,
    reporter: Option<Box<dyn crate::reporter::TestReporterInternal>>,
}

impl ParallelRunner {
    pub fn new(max_jobs: usize) -> Self {
        Self {
            max_jobs,
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
        }
    }

    pub fn new_default() -> Self {
        Self {
            max_jobs: num_cpus::get(),
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
        }
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
    suite: Option<Box<dyn TestSuiteInternal>>,
    reporter: Option<Box<dyn crate::reporter::TestReporterInternal>>,
}

impl ParallelRunnerBuilder {
    pub fn new() -> Self {
        Self {
            max_jobs: num_cpus::get(),
            shuffle_test_cases: false,
            suite: None,
            reporter: None,
        }
    }

    pub fn with_max_jobs(mut self, n: usize) -> Self {
        self.max_jobs = n;
        self
    }

    pub fn shuffle_test_cases(mut self) -> Self {
        self.shuffle_test_cases = true;
        self
    }

    pub fn with_suite(mut self, suite: Box<dyn crate::suite::TestSuiteInternal>) -> Self {
        self.suite = Some(suite);
        self
    }

    pub fn with_reporter(
        mut self,
        reporter: Box<dyn crate::reporter::TestReporterInternal>,
    ) -> Self {
        self.reporter = Some(reporter);
        self
    }

    pub fn build(self) -> ParallelRunner {
        ParallelRunner {
            max_jobs: self.max_jobs,
            shuffle_test_cases: self.shuffle_test_cases,
            suite: self.suite,
            reporter: self.reporter,
        }
    }
}

impl Default for ParallelRunnerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::runner::TestRunnerInternal for ParallelRunner {
    fn with_suite(mut self, suite: Box<dyn TestSuiteInternal>) -> Self {
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

            let ctx = suite.context().cloned();
            let suite_duration_start = Instant::now();

            let mut queued: VecDeque<crate::runner::CaseData> = test_cases
                .into_iter()
                .map(|case| {
                    let test_names = case
                        .tests()
                        .iter()
                        .map(|test| test.name().to_string())
                        .collect();
                    (case.name().to_string(), test_names, case)
                })
                .collect();

            if self.shuffle_test_cases {
                shuffle_queue(queued.make_contiguous(), &mut rand::rng());
            }

            let max_jobs = self.max_jobs;
            let mut running = JoinSet::new();

            // Start initial batch
            for _ in 0..max_jobs.min(queued.len()) {
                let (case_name, test_names, case) =
                    queued.pop_front().expect("queued case should exist");
                let ctx = ctx.clone();
                running.spawn(run_test_case(case_name, test_names, case, ctx));
            }

            // Process completions, start next in queue order
            while let Some(res) = running.join_next().await {
                if let Some((case_name, test_names, case)) = queued.pop_front() {
                    let ctx = ctx.clone();
                    running.spawn(run_test_case(case_name, test_names, case, ctx));
                }

                match res {
                    Ok(Ok(payload)) => {
                        reporter
                            .report_case_start(&payload.name, payload.test_names.len())
                            .await;

                        for (test_name, result) in payload.results {
                            reporter.report_test_start(&payload.name, &test_name).await;
                            reporter.report_result(&payload.name, &result).await;

                            if result.status == crate::report::TestStatus::Failed {
                                break;
                            }
                        }

                        reporter
                            .report_case_finish(
                                &payload.name,
                                payload.duration,
                                payload.total_duration,
                            )
                            .await;
                    }
                    Ok(Err(error_msg)) => {
                        reporter.report_case_start("unknown", 0).await;
                        reporter
                            .report_result(
                                "unknown",
                                &TestResult::failed(format!("test case panicked: {}", error_msg)),
                            )
                            .await;
                        reporter
                            .report_case_finish("unknown", Duration::ZERO, Duration::ZERO)
                            .await;
                    }
                    Err(join_error) => {
                        let join_error: tokio::task::JoinError = join_error;
                        reporter.report_case_start("unknown", 0).await;
                        reporter
                            .report_result(
                                "unknown",
                                &TestResult::failed(format!("test case panicked: {}", join_error)),
                            )
                            .await;
                        reporter
                            .report_case_finish("unknown", Duration::ZERO, Duration::ZERO)
                            .await;
                    }
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
                .report_finish(suite_duration, suite_total_duration)
                .await;
            reporter.get_report().clone()
        })
    }
}

fn shuffle_queue<T, R: Rng + ?Sized>(queue: &mut [T], rng: &mut R) {
    queue.shuffle(rng);
}

async fn run_test_case(
    case_name: String,
    test_names: Vec<String>,
    mut case: Box<dyn crate::case::TestCaseInternal>,
    ctx: Option<crate::report::TestContext>,
) -> Result<crate::runner::CaseResult, String> {
    let case_total_start = Instant::now();
    case.setup_case(ctx.as_ref()).await;
    let case_duration_start = Instant::now();

    let mut results = Vec::new();

    for test in case.tests() {
        let test_start = Instant::now();
        let mut result = test.run(ctx.as_ref()).await;
        let duration = test_start.elapsed();

        result.duration = duration;
        result.total_duration = duration;

        results.push((test.name().to_string(), result));

        if results.last().unwrap().1.status == crate::report::TestStatus::Failed {
            break;
        }
    }

    let duration = case_duration_start.elapsed();
    case.teardown_case(ctx.as_ref()).await;

    Ok(crate::runner::CaseResult {
        name: case_name,
        test_names,
        results,
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
