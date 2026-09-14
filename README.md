# rut

`rut` is a Rust testing framework similar to Python's `unittest`. It provides familiar concepts such as suites, cases, tests, setup, and teardown, together with a declarative syntax that removes manual trait implementation boilerplate. It supports sequential and bounded parallel execution with optional randomized case dispatch. Its modular design makes it easy to add custom runners and output formats.

## Getting Started

* Add `rut` to the list of your dependencies to write tests

```toml
[dev-dependencies]
rut = { git = "https://github.com/DexterMagnific/rut" }
tokio = { version = "1.40", features = ["macros", "rt-multi-thread"] }
```

* Install the Cargo subcommand to run tests

```console
cargo install --git https://github.com/DexterMagnific/rut cargo-rut
```

## Cargo rut

Use `cargo rut` to run test suites

* All suites from the current dir

```console
cargo rut run
```

* Suites from a specific file or directory

```console
cargo rut run path/to/suite.rs
cargo rut run path/to/suites/
```

* Run a specific suite by its typename

```console
cargo rut run --typename CalculatorSuite
cargo rut run path/to/suites/ --typename CalculatorSuite
```

When a source file declares multiple suites, all of them run unless `--typename` selects one.

* Run tests matching a qualified-name substring

```console
cargo rut run --filter addition
cargo rut run --filter Calculator.addition.positive
cargo rut run --filter addition --filter multiplication
```

Repeated filters are combined with OR. Matching uses the full `Suite.case.test` name, and tests
that do not match are excluded from the report.

* Stop admitting new cases after the first failure

```console
cargo rut run --fail-fast
```

The sequential runner stops before the next case. The parallel runner lets already-running cases
finish but does not dispatch more queued cases.

* List suites

```console
cargo rut list
cargo rut list path/to/suites/
```

* Save JUnit report

Exactly one selected suite: `--junit`

```console
cargo rut run path/to/suite.rs --typename CalculatorSuite --junit target/junit/calculator.xml
```

One or more selected suites: `--junit-dir`

```console
cargo rut run path/to/suite.rs --junit-dir target/junit
cargo rut run path/to/suites/ --junit-dir target/junit
```

* Save Google Test JSON report

```console
cargo rut run path/to/suite.rs --typename CalculatorSuite --gtest target/gtest/calculator.json
```

One or more selected suites: `--gtest-dir`

```console
cargo rut run path/to/suite.rs --gtest-dir target/gtest
cargo rut run path/to/suites/ --gtest-dir target/gtest
```

* JUnit+GTest:

```console
cargo rut run path/to/suites/ --junit-dir target/junit --gtest-dir target/gtest
```

Directory reports mirror the source path.

## Suites, Cases and Tests

A suite contains related test cases, and each case contains one or more tests.

The code below declares a test suite with two test cases.

```rust
use rut::{TestResult, suite};

suite! {
    // This is the Rust type generated for the suite
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
```

* Suites run with `cargo rut` do not need a `main` function.
* Returning `TestResult::failed` stops the remaining tests in that case. Other cases can still run.
* Panics inside test bodies are converted into failed test results with captured location and backtrace.

Tests can also be skipped explicitly with a reason:

```rust
test(name = "requires database") {
    if database_is_available() {
        TestResult::passed()
    } else {
        TestResult::skipped("database is unavailable")
    }
}
```

Skipped tests do not count as failures and retain their reason in JUnit and GoogleTest reports.

## Setup and Teardown

Setup and teardown are supported at both suite and case level.

```rust
use rut::{TestResult, suite};

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

Arbitrary properties (key-value pairs) can be attached to tests. Static properties are declared with the test, while dynamic properties can be set by the test code.

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

Properties are propagated to the test reporter which may or may not include them in the final output.

## Timeouts

Tests can declare a `timeout`, in milliseconds, using the same reserved-property syntax as `name`:

```rust
test(name = "responds quickly", timeout = "500") {
    some_async_call().await;
    TestResult::passed()
}
```

If the test does not complete within the declared timeout, it is reported with a `TimedOut`
status instead of running to completion, and counts as a failure in the suite report.

Timeouts rely on the test cooperatively yielding control (e.g. at `.await` points), the same way
panics are caught. A test body that never yields (a tight CPU-bound loop with no `.await`) cannot
be interrupted and will not be stopped by its timeout.

## Retries

Tests can declare a retry budget using the same reserved-property pattern as `name` and `timeout`:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

static FLAKY_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

test(name = "succeeds eventually", retries = "3") {
    let attempt = FLAKY_ATTEMPTS.fetch_add(1, Ordering::SeqCst) + 1;
    if attempt < 3 {
        TestResult::failed("still flaky")
    } else {
        TestResult::passed()
    }
}
```

When a test fails, `rut` retries it until it either passes or exhausts the configured retry budget.
A successful retry produces a final result with status `Unstable` and includes `failed_attempts`
to record how many earlier attempts failed before the eventual success.

If the test never succeeds within the retry limit, the final status remains `Failed` and the
retry metadata is not promoted to an `Unstable` result. The retry count is interpreted as the
number of additional attempts after the initial run, so `retries = "3"` allows a total of 4
executions.

## Custom Context

Optional suite-wide context can be provided when tests need shared state.

```rust
use rut::{TestResult, suite};
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

`rut` separates test definitions from their execution strategy. A **runner** takes a suite, executes its lifecycle and tests, sends progress to a reporter, and returns the completed report.

The framework includes two built-in runners: `SequentialRunner` executes one case at a time, while `ParallelRunner` executes independent cases concurrently. Both consume the same generated suite and return the same report type.

Use the sequential runner for predictable, one-case-at-a-time ordering:

```console
cargo rut run path/to/suite.rs --runner sequential
```

Use the parallel runner to execute independent cases concurrently. By default, the machine's CPU count is used as the concurrency limit and cases are dispatched in their declaration order:

```console
cargo rut run path/to/suite.rs --runner parallel
```

Use `--jobs` to choose the maximum number of concurrent cases:

```console
cargo rut run path/to/suite.rs --runner parallel --jobs 4
```

Parallel execution happens across test cases. Tests inside a single case remain sequential, and both runners keep suite setup and teardown around the complete run.

Add `--shuffle` to randomize case dispatch:

```console
cargo rut run path/to/suite.rs --runner parallel --jobs 4 --shuffle
```

Shuffling changes dispatch order only. Tests within each case still run sequentially, and `SuiteReport.test_cases` remains in declaration order so report consumers receive a stable structure.

## Manual Run

For custom suite execution and report post-processing, add your own `main` function after the `suite!` declaration. Note: you cannot use `cargo rut` in that case, so you have to manually invoke your test executable.

```rust
use rut::{ParallelRunnerBuilder, ReporterResult, StdoutReporter, TestRunner};

#[tokio::main]
async fn main() -> ReporterResult<()> {
    let report = ParallelRunnerBuilder::new()
        .with_max_jobs(4)
        .shuffle_test_cases()
        .with_suite(Box::new(CalculatorSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .build()
        .run()
        .await?;

    println!("{} tests passed", report.total_passed);
    Ok(())
}
```

## Reporters

Reporting is modular too. A **reporter** receives events as the runner starts and finishes suites, cases, and tests. It decides how progress is presented and builds the final `SuiteReport`.

`StdoutReporter` is the built-in terminal reporter. It prints live progress, pass/fail results, durations, failure messages, and properties:

```rust
use rut::{ParallelRunner, StdoutReporter, TestRunner};

let report = ParallelRunner::default()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await?;
```

The built-in `MultiReporter` forwards runner events in order and returns the first reporter's completed report.

```rust
use rut::{
    GTestReporter, JUnitReporter, MultiReporter, ParallelRunner, StdoutReporter,
    TestRunner,
};

let reporter = MultiReporter::new()
    .add_reporter(Box::new(StdoutReporter::new()))
    .add_reporter(Box::new(JUnitReporter::new("target/junit/results.xml")))
    .add_reporter(Box::new(GTestReporter::new("target/gtest/results.json")));

let report = ParallelRunner::default()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(reporter))
    .run()
    .await?;
```

The resulting `SuiteReport` can be explored for post processing:

```rust
println!("Suite: {}", report.suite_name);
println!("Passed: {}", report.total_passed);
println!("Failed: {}", report.total_failed);
println!("Skipped: {}", report.total_skipped);
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

More complete examples are available in [`rut/examples/declarative.rs`](rut/examples/declarative.rs), [`rut/examples/sequential.rs`](rut/examples/sequential.rs), and [`rut/examples/parallel.rs`](rut/examples/parallel.rs).
