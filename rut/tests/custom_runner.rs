use async_trait::async_trait;
use rut::{
    MultiReporter, ParallelRunner, ParallelRunnerBuilder, ReporterResult, SequentialRunner,
    StdoutReporter, SuiteReport, TestReporter, TestResult, TestRunner, TestSuite, suite,
};

suite! {
    typename = CustomRunnerSuite;
    name = "custom runner suite";

    test_case(name = "case") {
        test(name = "passes") {
            TestResult::passed()
        }
    }
}

struct CustomRunner {
    suite: Option<Box<dyn TestSuite>>,
    reporter: Option<Box<dyn TestReporter>>,
}

impl CustomRunner {
    fn new() -> Self {
        Self {
            suite: None,
            reporter: None,
        }
    }
}

#[async_trait]
impl TestRunner for CustomRunner {
    fn set_suite(&mut self, suite: Box<dyn TestSuite>) {
        self.suite = Some(suite);
    }

    fn set_reporter(&mut self, reporter: Box<dyn TestReporter>) {
        self.reporter = Some(reporter);
    }

    async fn run(self) -> ReporterResult<SuiteReport> {
        SequentialRunner::new()
            .with_suite(self.suite.expect("suite required"))
            .with_reporter(self.reporter.expect("reporter required"))
            .run()
            .await
    }
}

#[tokio::test]
async fn custom_runner_uses_only_public_traits() {
    let report = CustomRunner::new()
        .with_suite(Box::new(CustomRunnerSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    assert_eq!(report.total_passed, 1);
    assert_eq!(report.total_failed, 0);
}

/// Mirrors the manual-run example in the README.
#[tokio::test]
async fn builder_runs_a_suite_without_the_harness() {
    let report = ParallelRunnerBuilder::new()
        .with_max_jobs(4)
        .shuffle_test_cases()
        .with_suite(Box::new(CustomRunnerSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .build()
        .run()
        .await
        .unwrap();

    assert_eq!(report.total_passed, 1);
}

/// Mirrors the reporter example in the README.
#[tokio::test]
async fn default_runner_accepts_a_composed_reporter() {
    let reporter = MultiReporter::new().add_reporter(Box::new(StdoutReporter::new()));

    let report = ParallelRunner::default()
        .with_suite(Box::new(CustomRunnerSuite::new()))
        .with_reporter(Box::new(reporter))
        .run()
        .await
        .unwrap();

    assert_eq!(report.total_passed, 1);
}
