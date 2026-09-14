use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rut::{ParallelRunnerBuilder, SequentialRunner, TestResult, TestRunnerInternal, suite};

struct Context {
    ready: bool,
}

suite! {
    typename = TypedContextSuite;
    name = "typed context";
    context = Context;

    setup {
        tokio::task::yield_now().await;
        context.set(Context { ready: true });
    }

    test_case(name = "case") {
        test(name = "sees setup context") {
            if context.ready {
                TestResult::passed()
            } else {
                TestResult::failed("context was not initialized")
            }
        }
    }
}

suite! {
    typename = ContextFreeSuite;
    name = "context free";

    test_case(name = "case") {
        test(name = "runs without context") {
            TestResult::passed()
        }
    }
}

static MISSING_CONTEXT_TEST_RAN: AtomicBool = AtomicBool::new(false);

suite! {
    typename = MissingContextSuite;
    name = "missing context";
    context = Context;

    test_case(name = "case") {
        test(name = "must not run") {
            MISSING_CONTEXT_TEST_RAN.store(true, Ordering::SeqCst);
            TestResult::passed()
        }
    }
}

suite! {
    typename = TimingSuite;
    name = "runner timing";

    setup {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    test_case(name = "timed case") {
        setup {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        test(name = "timed test") {
            tokio::time::sleep(Duration::from_millis(10)).await;
            TestResult::passed()
        }

        teardown {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    teardown {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

suite! {
    typename = SetupFailureTimingSuite;
    name = "setup failure timing";

    setup {
        tokio::time::sleep(Duration::from_millis(10)).await;
        panic!("setup failed");
    }

    test_case(name = "unreachable case") {
        test(name = "unreachable test") {
            TestResult::passed()
        }
    }
}

fn assert_runner_durations(report: &rut::SuiteReport) {
    let case = &report.test_cases[0];
    let test = &case.tests[0];

    assert!(test.duration >= Duration::from_millis(10));
    assert_eq!(test.duration, test.total_duration);

    assert!(case.duration >= test.total_duration);
    assert!(case.total_duration >= Duration::from_millis(30));
    assert!(case.total_duration - case.duration >= Duration::from_millis(20));

    assert!(report.duration >= case.total_duration);
    assert!(report.total_duration >= Duration::from_millis(50));
    assert!(report.total_duration - report.duration >= Duration::from_millis(20));
}

fn assert_setup_failure_durations(report: &rut::SuiteReport) {
    assert_eq!(report.duration, Duration::ZERO);
    assert!(report.total_duration >= Duration::from_millis(10));
}

#[tokio::test]
async fn setup_initializes_typed_context_for_sequential_tests() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(TypedContextSuite::new()))
        .run()
        .await;

    assert_eq!(report.total_passed, 1);
    assert_eq!(report.total_failed, 0);
    assert_eq!(report.test_cases[0].tests[0].name, "sees setup context");
}

#[test]
fn result_constructors_do_not_require_a_test_name() {
    let passed = TestResult::passed();
    let failed = TestResult::failed("expected value");

    assert!(passed.name.is_empty());
    assert!(failed.name.is_empty());
    assert_eq!(failed.message.as_deref(), Some("expected value"));
}

#[tokio::test]
async fn setup_context_is_shared_with_parallel_tests() {
    let report = ParallelRunnerBuilder::new()
        .with_max_jobs(2)
        .with_suite(Box::new(TypedContextSuite::new()))
        .build()
        .run()
        .await;

    assert_eq!(report.total_passed, 1);
    assert_eq!(report.total_failed, 0);
}

#[tokio::test]
async fn suite_without_context_runs_normally() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(ContextFreeSuite::new()))
        .run()
        .await;

    assert_eq!(report.total_passed, 1);
    assert_eq!(report.total_failed, 0);
}

#[tokio::test]
async fn missing_declared_context_stops_before_tests() {
    MISSING_CONTEXT_TEST_RAN.store(false, Ordering::SeqCst);

    let report = SequentialRunner::new()
        .with_suite(Box::new(MissingContextSuite::new()))
        .run()
        .await;

    assert!(!MISSING_CONTEXT_TEST_RAN.load(Ordering::SeqCst));
    assert_eq!(report.total_passed, 0);
}

#[tokio::test]
async fn sequential_runner_measures_all_lifecycle_durations() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(TimingSuite::new()))
        .run()
        .await;

    assert_runner_durations(&report);
}

#[tokio::test]
async fn parallel_runner_measures_all_lifecycle_durations() {
    let report = ParallelRunnerBuilder::new()
        .with_suite(Box::new(TimingSuite::new()))
        .build()
        .run()
        .await;

    assert_runner_durations(&report);
}

#[tokio::test]
async fn sequential_runner_records_failed_setup_total_duration() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(SetupFailureTimingSuite::new()))
        .run()
        .await;

    assert_setup_failure_durations(&report);
}

#[tokio::test]
async fn parallel_runner_records_failed_setup_total_duration() {
    let report = ParallelRunnerBuilder::new()
        .with_suite(Box::new(SetupFailureTimingSuite::new()))
        .build()
        .run()
        .await;

    assert_setup_failure_durations(&report);
}
