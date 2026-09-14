use rut::{
    ParallelRunnerBuilder, SequentialRunner, StdoutReporter, TestResult, TestRunner, TestStatus,
    suite,
};

suite! {
    typename = FailFastSuite;
    name = "fail fast suite";

    test_case(name = "first") {
        test(name = "fails") {
            TestResult::failed("stop")
        }
    }

    test_case(name = "second") {
        test(name = "would pass") {
            TestResult::passed()
        }
    }

    test_case(name = "third") {
        test(name = "would also pass") {
            TestResult::passed()
        }
    }
}

fn assert_stopped_after_first_case(report: &rut::SuiteReport) {
    assert_eq!(report.total_failed, 1);
    assert_eq!(report.total_passed, 0);
    assert_eq!(report.test_cases[0].status, TestStatus::Failed);
    assert_eq!(report.test_cases[1].status, TestStatus::NotYetRun);
    assert_eq!(report.test_cases[2].status, TestStatus::NotYetRun);
}

#[tokio::test]
async fn sequential_runner_stops_before_the_next_case() {
    let report = SequentialRunner::new()
        .fail_fast()
        .with_suite(Box::new(FailFastSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    assert_stopped_after_first_case(&report);
}

#[tokio::test]
async fn parallel_runner_stops_admitting_queued_cases() {
    let report = ParallelRunnerBuilder::new()
        .with_max_jobs(1)
        .fail_fast()
        .with_suite(Box::new(FailFastSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .build()
        .run()
        .await
        .unwrap();

    assert_stopped_after_first_case(&report);
}
