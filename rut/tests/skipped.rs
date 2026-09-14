use rut::{SequentialRunner, StdoutReporter, TestResult, TestRunner, TestStatus, suite};

suite! {
    typename = SkippedSuite;
    name = "skipped suite";

    test_case(name = "optional") {
        test(name = "requires service") {
            TestResult::skipped("service is unavailable")
        }
    }
}

#[tokio::test]
async fn runner_reports_intentional_skips_without_failure() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(SkippedSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    assert_eq!(report.total_passed, 0);
    assert_eq!(report.total_failed, 0);
    assert_eq!(report.total_skipped, 1);
    assert_eq!(report.test_cases[0].skipped, 1);
    assert_eq!(report.test_cases[0].status, TestStatus::Skipped);
    assert_eq!(report.test_cases[0].tests[0].status, TestStatus::Skipped);
    assert_eq!(
        report.test_cases[0].tests[0].message.as_deref(),
        Some("service is unavailable")
    );
}
