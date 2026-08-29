# rut

`rut` is a Rust testing framework similar to Python Unit Test. It provides the same testing concepts such as suites, cases, tests, setup/teardown in addition to a friendly declarative syntax for writing tests eliminating all the rust manual boilerplate of trait implementations. It supports sequential execution and bounded parallel execution with optional randomized case dispatch. It is designed with modularity in mind so that adding custom runners and output formats is made easy.

## Getting Started

Until `rut` is published on crates.io, add it from GitHub together with Tokio:

```toml
[dependencies]
rut = { git = "https://github.com/DexterMagnific/rut" }
tokio = { version = "1.40", features = ["macros", "rt-multi-thread"] }
```

## Suites, Cases and Tests

A suite contains related test cases, and each case contains one or more tests. 

The code below declares a test suite with two test cases.

```rust
use rut::{ParallelRunner, StdoutReporter, TestResult, TestRunnerInternal, suite};

suite! {
    // This is the Rust type given to the suite. Needed for instanciating it
    typename = CalculatorSuite;
    // This is the display name of the suite. Will make its way to the suite report
    name = "Calculator";

    // Declare a test case with its display name
    test_case(name = "addition") {
        // Declare a test inside this test case
        test(name = "adds positive numbers") {
            let actual = 2 + 2;

            if actual == 4 {
                // PASS
                TestResult::passed()
            } else {
                // FAIL, with an error message
                TestResult::failed("expected 4")
            }
        }

        test(name = "adds negative numbers") {
            let actual = -2 + -3;

            if actual == -5 {
                TestResult::passed()
            } else {
                TestResult::failed("expected -5")
            }
        }
    }

    test_case(name = "multiplication") {
        test(name = "multiplies numbers") {
            // Tests are async capable by nature. No async keyword is needed
            tokio::task::yield_now().await;

            if 6 * 7 == 42 {
                TestResult::passed()
            } else {
                TestResult::failed("expected 42")
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Create a Runner to execute the tests
    let report = ParallelRunner::default()
        // Add the suite using its typename
        .with_suite(Box::new(CalculatorSuite::new()))
        // Add the Reporter that will write test results
        .with_reporter(Box::new(StdoutReporter::new()))
        // and run the suite
        .run()
        .await;

    // The run generates a SuiteReport. Explore it as you whish
    assert_eq!(report.total_failed, 0);
}
```

Returning `TestResult::failed` stops the remaining tests in that case. Other cases can still run.

## Setup and Teardown

Setup and Teardown are supported at both suite and case level.

```rust
suite! {
    typename = CalculatorSuite;
    name = "Calculator";

    // This is the setup code for the suite. It is async capable
    setup {
        tokio::task::yield_now().await;
        println!("prepare the suite");
    }

    test_case(name = "addition") {
        // This is the setup code for this case. It is async capable too
        setup {
            println!("prepare the addition case");
        }

        test(name = "adds positive numbers") {
            TestResult::passed()
        }

        // This is the teardown for this case
        teardown {
            println!("clean up the addition case");
        }
    }

    // This is the teardown for the suite
    teardown {
        println!("clean up the suite");
    }
}
```

## Properties

Arbitrary properties (key value pairs) can be attached to tests. Static properties are directly declared
with the test, and Dynamic properties can be set by the test code.

Properties are propagated to the test report and can be used by the reporter for display or file write.

```rust
test(
    // name is reserved for the test name
    name = "adds positive numbers",
    // Static properties
    category = "arithmetic",
    priority = "high"
) {
    let left = 2;
    let right = 2;
    let actual = left + right;

    TestResult::passed()
        // Dynamic properties
        .with_property("actual", actual.to_string())
        .with_property("inputs", format!("{left}, {right}"))
}
```

## Custom Context

Optional suite wide context can be provided when tests need shared state.

```rust
use std::sync::{Arc, Mutex};

// A sample context struct
#[derive(Clone, Default)]
struct AppContext {
    // Concurrent context access requires appropriate protections
    events: Arc<Mutex<Vec<String>>>,
}

impl AppContext {
    fn record(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }
}

suite! {
    typename = CalculatorSuite;
    name = "Calculator";
    // Declare the suite context type
    context = AppContext;

    setup {
        let app = AppContext::default();
        app.record("suite setup");
        // The suite context is accessible through the 'context' variable automatic binding
        // The set() method fills initial context inside the suite's setup
        context.set(app);
    }

    test_case(name = "addition") {
        setup {
            // Context is accessible everywhere
            context.record("addition setup");
        }

        test(name = "adds positive numbers") {
            // In tests too
            context.record("addition test");
            TestResult::passed()
        }
    }

    teardown {
        // Even in teardown
        context.record("suite teardown");
    }
}
```

## Sequential or Parallel?

`rut` separates test definitions from their execution strategy. A **runner** takes a suite, executes its lifecycle and tests, sends progress to a reporter, and returns the completed report. Because the suite DSL is independent of the runner, the same suite can run with different execution strategies without changing any tests.

The framework includes two built-in runners: `SequentialRunner` executes one case at a time, while `ParallelRunner` executes independent cases concurrently. Both consume the same generated suite and return the same report type.

Use the sequential runner for single thread predictable case ordering:

```rust
use rut::{SequentialRunner, StdoutReporter, TestRunnerInternal};

let report = SequentialRunner::new()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await;
```

Use the parallel runner to execute independent cases concurrently. By default, the machine's CPU count is used as the concurrency limit and cases are dispatched in their declaration order:

```rust
use rut::{ParallelRunner, StdoutReporter, TestRunnerInternal};

let report = ParallelRunner::default()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await;
```

Use `ParallelRunnerBuilder` to choose the maximum number of concurrent cases and optionally randomize case dispatch:

```rust
use rut::{ParallelRunnerBuilder, StdoutReporter, TestRunnerInternal};

let report = ParallelRunnerBuilder::new()
    .with_max_jobs(4)
    .shuffle_test_cases()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .build()
    .run()
    .await;
```

Parallel execution happens across test cases. Tests inside a single case remain sequential, and both runners keep suite setup and teardown around the complete run.

`shuffle_test_cases()` is opt-in. When enabled, the runner shuffles the complete case queue once with fresh randomness before assigning cases to workers. This can expose dependencies between cases that declaration-order execution might hide. Omit the method when reproducible declaration-order dispatch is preferred.

Shuffling changes dispatch order only. Tests within each case still run sequentially, and `SuiteReport.test_cases` remains in declaration order so report consumers receive a stable structure.

## Reporters

Reporting is modular too. A **reporter** receives events as the runner starts and finishes suites, cases, and tests. It decides how progress is presented and builds the final `SuiteReport`.

`StdoutReporter` is the built-in terminal reporter. It prints live progress, pass/fail results, durations, failure messages, and properties:

```rust
let report = ParallelRunner::default()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await;
```

Another interesting reporter is the `MultiReporter` pseudo reporter. It forwards the runner's events to all its sub-reporters, which allows for exemple to use both the StdoutReporter for console printing and another one for JUnit or GTest file write.

The runner returns the reporter's completed `SuiteReport`, so results remain available for assertions or further processing after output is written. It contains suite totals and a result for every case and test:

Every suite, case, and test result has two timing fields. `duration` excludes that item's own setup and teardown, while `total_duration` includes them. A suite's `duration` therefore includes complete case lifecycles, and a case's `duration` includes its complete test executions. Tests currently have equal values for both fields because they do not have test-level setup or teardown. Parallel suite timings are elapsed wall-clock durations, not sums of concurrently running cases.

```rust
println!("Suite: {}", report.suite_name);
println!("Passed: {}", report.total_passed);
println!("Failed: {}", report.total_failed);
println!("Duration: {:?}", report.duration);
println!("Total duration: {:?}", report.total_duration);

for case in &report.test_cases {
    println!(
        "Case: {} ({:?}, {} passed, {} failed, duration: {:?}, total duration: {:?})",
        case.name,
        case.status,
        case.passed,
        case.failed,
        case.duration,
        case.total_duration
    );

    for test in &case.tests {
        println!(
            "  Test: {} ({:?}, duration: {:?}, total duration: {:?})",
            test.name, test.status, test.duration, test.total_duration
        );

        if let Some(message) = &test.message {
            println!("    Message: {message}");
        }

        for (key, value) in &test.properties {
            println!("    {key} = {value}");
        }
    }
}
```

## Rust syntax, the hard way

For those who are not comfortable with the declarative syntax, all of the above can be achieved using classic
Rust trait implementations.

More complete examples are available in [`rut/examples/attributes.rs`](rut/examples/attributes.rs), [`rut/examples/sequential.rs`](rut/examples/sequential.rs), and [`rut/examples/parallel.rs`](rut/examples/parallel.rs).
