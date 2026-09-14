# rut

`rut` is a Rust testing framework similar to Python's `unittest`. It provides familiar concepts such as suites, cases, tests, setup, and teardown, together with a declarative syntax that removes manual trait implementation boilerplate. It supports sequential and bounded parallel execution with optional randomized case dispatch. Its modular design makes it easy to add custom runners and output formats.

## Getting Started

Until `rut` is published on crates.io, add it from GitHub together with Tokio:

```toml
[dev-dependencies]
rut = { git = "https://github.com/DexterMagnific/rut" }
tokio = { version = "1.40", features = ["macros", "rt-multi-thread"] }
```

Install the Cargo subcommand:

```console
cargo install --git https://github.com/DexterMagnific/rut cargo-rut
```

## Cargo rut

Run every suite in the current project:

```console
cargo rut run
```

Run suites from a file or directory:

```console
cargo rut run path/to/suite.rs
cargo rut run path/to/suites/
```

Run a specific suite by its Rust typename:

```console
cargo rut run --typename CalculatorSuite
cargo rut run path/to/suites/ --typename CalculatorSuite
```

The optional path narrows the typename search. If the same typename exists in more than one discovered file, provide a file or narrower directory to select one.

List every discovered suite with its file, display name, typename, cases, and test counts:

```console
cargo rut list
cargo rut list path/to/suites/
```

```text
path/to/suites/calculator.rs
    Calculator (CalculatorSuite): 2 cases
        addition: 1 test
        multiplication: 1 test
```

Write JUnit XML for a single selected suite:

```console
cargo rut run path/to/suite.rs --junit target/junit/calculator.xml
cargo rut run --typename CalculatorSuite --junit target/junit/calculator.xml
```

`--junit FILE` requires exactly one selected suite and overwrites `FILE`. Live terminal output remains enabled.

For one or many suites, use an output directory:

```console
cargo rut run path/to/suites/ --junit-dir target/junit
```

GoogleTest-compatible JSON supports the same file and directory modes:

```console
cargo rut run path/to/suite.rs --gtest target/gtest/calculator.json
cargo rut run path/to/suites/ --gtest-dir target/gtest
```

JUnit and GoogleTest output can be generated in the same run:

```console
cargo rut run path/to/suites/ --junit-dir target/junit --gtest-dir target/gtest
```

Directory reports use the predictable path `<report-dir>/<source-relative-parent>/<suite-slug>.<extension>`. Source subdirectories are mirrored, existing files are overwritten, and conflicting suite slugs in the same directory are rejected before execution.

Suites run with `cargo rut` do not need a `main` function. See [`rut/examples/rut_suite.rs`](rut/examples/rut_suite.rs) for a complete example.

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

Returning `TestResult::failed` stops the remaining tests in that case. Other cases can still run.

`TestResult::failed` records the source location where it is called. It uses Rust's
`#[track_caller]`, so helper functions that should preserve their caller's location must also be
annotated with `#[track_caller]`:

```rust
#[track_caller]
fn failure(message: impl Into<String>) -> TestResult {
    TestResult::failed(message)
}
```

Panics inside test bodies are converted into failed test results. Their failure location is the
panic origin, and their message contains the panic payload followed by a force-captured stack
backtrace. A panic therefore appears to reporters like a detailed failure string instead of
aborting the suite. Panics in suite or case setup and teardown are not yet modeled as test
failures.

## Setup and Teardown

Setup and teardown are supported at both suite and case level.

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

Arbitrary properties (key-value pairs) can be attached to tests. Static properties are declared with the test, while dynamic properties can be set by the test code.

Properties are propagated to the test report. `JUnitReporter` writes them under their owning `<testcase>` as JUnit `<property>` elements. `GTestReporter` flattens them onto the test object with a `prop_` prefix; if a key occurs more than once, the last value wins.

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

Optional suite-wide context can be provided when tests need shared state.

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

Use the sequential runner for predictable, single-threaded case ordering:

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

For custom suite execution and report post-processing, add your own `main` function after the `suite!` declaration.

```rust
use rut::{ParallelRunnerBuilder, ReporterResult, StdoutReporter, TestRunnerInternal};

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

Each test result has two distinct locations:

- `source` identifies the `test(...)` declaration.
- `failure_location` identifies the `TestResult::failed(...)` call or panic origin.

Macro-generated tests obtain their declaration path from the compiler's `file!()` value. Paths
are stored without runtime canonicalization, and line and byte-column numbers are one-based.
Hand-written `Test` implementations can override `source_location`; the default returns `None`.

`StdoutReporter` is the built-in terminal reporter. It prints live progress, pass/fail results, durations, failure messages, and properties:

```rust
let report = ParallelRunner::default()
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await?;
```

`JUnitReporter` writes JUnit XML and `GTestReporter` writes GoogleTest-compatible JSON. Combine either or both with `StdoutReporter` to keep live terminal output:

```rust
use rut::{GTestReporter, JUnitReporter, MultiReporter, StdoutReporter};

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

`MultiReporter` forwards runner events in order and returns the first reporter's completed report. Reporter callbacks are fallible, so serialization, directory creation, and write failures are returned by `.run().await` instead of being silently ignored. Both file reporters overwrite their destinations and record unfinished tests as skipped.

`ReporterResult<T>` uses a type-erased `anyhow::Error`, so a custom reporter is not limited to
framework-defined error variants. Standard errors work with `?`, and any custom error implementing
`std::error::Error + Send + Sync + 'static` can be returned with `.into()`:

```rust
#[derive(Debug)]
struct UploadError;

impl std::fmt::Display for UploadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("report upload failed")
    }
}

impl std::error::Error for UploadError {}

async fn upload_report() -> Result<(), UploadError> {
    // Upload the completed report.
    Ok(())
}

async fn finish_custom_reporter() -> ReporterResult<()> {
    upload_report().await?;
    Ok(())
}
```

Applications with a direct `anyhow` dependency can use `anyhow::Context` to add operation or path
details while preserving the original error for chain inspection and downcasting. Code migrating
from `ReporterError` should return its native errors with context instead of constructing or
matching format-specific variants.

`StdoutReporter` displays declaration and failure locations. `GTestReporter` writes declaration
`file` and `line` fields plus a rut `column` extension. `JUnitReporter` writes `file`, `line`, and
`column` attributes on `<testcase>` as common xUnit extensions. Both file reporters prefix failure
text with its failure location and preserve panic backtraces in the failure body.

GoogleTest JSON does not have a stable formal specification. `GTestReporter` follows GoogleTest's current JSON test-output shape. Rut does not emit parameter metadata, per-test timestamps, disabled counts, or error counts because those values are not present in `SuiteReport`.

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

More complete examples are available in [`rut/examples/declarative.rs`](rut/examples/declarative.rs), [`rut/examples/sequential.rs`](rut/examples/sequential.rs), and [`rut/examples/parallel.rs`](rut/examples/parallel.rs).
