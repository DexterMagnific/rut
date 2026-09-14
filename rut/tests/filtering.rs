use rut::{ParallelRunnerBuilder, SequentialRunner, StdoutReporter, TestResult, TestRunner, suite};

suite! {
    typename = FilterSuite;
    name = "calculator";

    test_case(name = "addition") {
        test(name = "positive") {
            TestResult::passed()
        }

        test(name = "negative") {
            TestResult::passed()
        }
    }

    test_case(name = "multiplication") {
        test(name = "positive") {
            TestResult::passed()
        }
    }
}

#[tokio::test]
async fn sequential_runner_filters_qualified_test_names() {
    let report = SequentialRunner::new()
        .with_filter("addition.positive")
        .with_suite(Box::new(FilterSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await
        .unwrap();

    assert_eq!(report.test_cases.len(), 1);
    assert_eq!(report.test_cases[0].name, "addition");
    assert_eq!(report.test_cases[0].tests.len(), 1);
    assert_eq!(report.test_cases[0].tests[0].name, "positive");
    assert_eq!(report.total_passed, 1);
}

#[tokio::test]
async fn parallel_runner_combines_repeated_filters() {
    let report = ParallelRunnerBuilder::new()
        .with_filter("addition.negative")
        .with_filter("multiplication")
        .with_suite(Box::new(FilterSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .build()
        .run()
        .await
        .unwrap();

    assert_eq!(report.test_cases.len(), 2);
    assert_eq!(report.test_cases[0].tests.len(), 1);
    assert_eq!(report.test_cases[0].tests[0].name, "negative");
    assert_eq!(report.test_cases[1].tests.len(), 1);
    assert_eq!(report.test_cases[1].tests[0].name, "positive");
    assert_eq!(report.total_passed, 2);
}
