use rut::{SequentialRunner, StdoutReporter, TestResult, TestRunner, TestStatus, suite};
use std::time::Duration;

suite! {
    typename = TimeoutSuite;
    name = "timeout suite";

    test_case(name = "within timeout") {
        test(name = "stays within timeout", timeout = "500") {
            TestResult::passed()
        }

        test(name = "has no timeout") {
            TestResult::passed()
        }
    }

    test_case(name = "exceeds timeout") {
        test(name = "exceeds timeout", timeout = "50") {
            tokio::time::sleep(Duration::from_millis(500)).await;
            TestResult::passed()
        }
    }
}

#[tokio::test]
async fn test_exceeding_its_timeout_is_reported_as_timed_out() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(TimeoutSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    let within_timeout_case = &report.test_cases[0];
    assert_eq!(within_timeout_case.status, TestStatus::Passed);
    assert_eq!(within_timeout_case.passed, 2);

    let exceeds_timeout_case = &report.test_cases[1];
    let timed_out = &exceeds_timeout_case.tests[0];

    assert_eq!(timed_out.status, TestStatus::TimedOut);
    assert!(timed_out.message.as_deref().unwrap().contains("timed out"));
    assert_eq!(report.total_failed, 1);
    assert_eq!(exceeds_timeout_case.failed, 1);
}

