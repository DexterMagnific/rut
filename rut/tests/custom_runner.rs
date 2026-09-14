use async_trait::async_trait;
use rut::{
    ReporterResult, SequentialRunner, StdoutReporter, SuiteReport, TestReporter, TestResult,
    TestRunner, TestSuite, suite,
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
    fn with_suite(mut self, suite: Box<dyn TestSuite>) -> Self {
        self.suite = Some(suite);
        self
    }

    fn with_reporter(mut self, reporter: Box<dyn TestReporter>) -> Self {
        self.reporter = Some(reporter);
        self
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
