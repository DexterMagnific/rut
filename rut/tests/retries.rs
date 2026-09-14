use std::sync::atomic::{AtomicU32, Ordering};

use rut::{SequentialRunner, StdoutReporter, TestResult, TestRunner, TestStatus, suite};

static FLAKY_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static FAILED_ATTEMPTS: AtomicU32 = AtomicU32::new(0);

suite! {
    typename = FlakyRetrySuite;
    name = "flaky retry suite";

    test_case(name = "flaky") {
        test(name = "passes after retries", retries = "3") {
            let attempt = FLAKY_ATTEMPTS.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 3 {
                TestResult::failed("still flaky")
            } else {
                TestResult::passed()
            }
        }
    }
}

suite! {
    typename = ExhaustedRetrySuite;
    name = "exhausted retry suite";

    test_case(name = "always fails") {
        test(name = "fails every time", retries = "2") {
            FAILED_ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            TestResult::failed("still broken")
        }
    }
}

#[tokio::test]
async fn flaky_tests_pass_after_retries_and_are_marked_unstable() {
    FLAKY_ATTEMPTS.store(0, Ordering::SeqCst);

    let report = SequentialRunner::new()
        .with_suite(Box::new(FlakyRetrySuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    let flaky = &report.test_cases[0].tests[0];
    assert_eq!(flaky.status, TestStatus::Unstable);
    assert_eq!(flaky.failed_attempts, 2);
    assert_eq!(report.total_passed, 1);
    assert_eq!(report.test_cases[0].passed, 1);
}

#[tokio::test]
async fn exhausted_retries_leave_the_final_failure_status() {
    FAILED_ATTEMPTS.store(0, Ordering::SeqCst);

    let report = SequentialRunner::new()
        .with_suite(Box::new(ExhaustedRetrySuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    let failed = &report.test_cases[0].tests[0];
    assert_eq!(failed.status, TestStatus::Failed);
    assert_eq!(failed.failed_attempts, 0);
    assert_eq!(report.total_failed, 1);
    assert_eq!(report.test_cases[0].failed, 1);
}
