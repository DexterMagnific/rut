use rut::{TestResult, TestRunnerInternal, suite};

suite! {
    typename = CalculatorSuite;
    name = "Calculator";

    test_case(name = "addition") {
        test(name = "adds positive numbers") {
            let result = 2 + 2;

            if result == 4 {
                TestResult::passed()
            } else {
                TestResult::failed(format!("expected 4, got {result}"))
            }
        }
    }

    test_case(name = "multiplication") {
        test(name = "multiplies numbers asynchronously") {
            tokio::task::yield_now().await;
            let result = 6 * 7;

            if result == 42 {
                TestResult::passed()
            } else {
                TestResult::failed(format!("expected 42, got {result}"))
            }
        }
    }
}