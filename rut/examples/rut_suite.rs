use rut::{TestResult, TestRunnerInternal, suite};

suite! {
    typename = CalculatorSuite;
    name = "Calculator";

    test_case(name = "addition") {
        test(
            name = "adds positive numbers",
            category = "arithmetic",
            operation = "addition"
        ) {
            let left = 2;
            let right = 2;
            let result = left + right;

            let outcome = if result == 4 {
                TestResult::passed()
            } else {
                TestResult::failed(format!("expected 4, got {result}"))
            };

            outcome
                .with_property("operands", format!("{left}, {right}"))
                .with_property("result", result.to_string())
        }
    }

    test_case(name = "multiplication") {
        test(
            name = "multiplies numbers asynchronously",
            category = "arithmetic",
            operation = "multiplication"
        ) {
            tokio::task::yield_now().await;
            let left = 6;
            let right = 7;
            let result = left * right;

            let outcome = if result == 42 {
                TestResult::passed()
            } else {
                TestResult::failed(format!("expected 42, got {result}"))
            };

            outcome
                .with_property("operands", format!("{left}, {right}"))
                .with_property("result", result.to_string())
        }
    }
}