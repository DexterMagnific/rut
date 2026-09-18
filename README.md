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

Reporters are selected with `--reporters`, or implicitly by passing one of their output options.

A single file with `--junit`:

```console
cargo rut run path/to/suite.rs --typename CalculatorSuite --junit target/junit/calculator.xml
```

One predictable file per selected suite with `--junit-dir`:

```console
cargo rut run path/to/suite.rs --junit-dir target/junit
cargo rut run path/to/suites/ --junit-dir target/junit
```

Every selected suite writes to the same file when `--junit` is used with more than one suite, so
prefer `--junit-dir` in that case.

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

* Choose reporters explicitly

```console
cargo rut run path/to/suite.rs --reporters=stdout,junit --junit-dir target/junit
```

Without `--reporters`, `stdout` is used, plus every reporter whose output option was given.

* See every available option, including the ones contributed by plugins

```console
cargo rut run path/to/suite.rs --help
```

* Pass arguments to the suite itself

```console
cargo rut run path/to/suite.rs --suite-args ip=10.0.0.1 port=8080
```

`--suite-args` must come last: every argument after it belongs to the suite and must be in
`KEY=VALUE` form. See [Suite Arguments](#suite-arguments).

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

### Declarative DSL reference

The `suite!` declaration has this shape:

```rust
suite! {
    typename = [pub] SuiteType;
    name = "Suite display name";
    context = ContextType;

    setup { /* optional async suite setup */ }

    test_case(name = "case display name") {
        setup { /* optional async case setup */ }

        test(
            name = "test display name",
            property = "static string value",
            timeout = "500",
            retries = "2"
        ) {
            TestResult::passed()
        }

        teardown { /* optional async case teardown */ }
    }

    teardown { /* optional async suite teardown */ }
}
```

The required header fields are `typename` and `name`. `typename` accepts an
optional visibility, such as `pub`, and names the generated suite type. The
suite must contain at least one `test_case`; every case must contain at least
one `test`. Suite and case names, test names, and property values are string
literals. Additional Rust helper items, such as structs and functions, may be
declared in suite and case bodies.

When `context = ContextType` is present, suite setup can initialize the shared
context with `context.set(value)`. The generated `context` binding is then the
typed value in suite teardown, case hooks, and test bodies. Context values must
be usable across the runner's tasks (`Send + Sync`).

Each test body is asynchronous and must return a `TestResult`. Static test
properties are forwarded to reporters. `timeout` is a positive number of
milliseconds and is cooperative: the test must yield at an `.await` point.
`retries` is the number of additional attempts after the initial execution. A
test that eventually passes is reported as `Unstable`; a test that exhausts
its retries remains `Failed`.

The macro reports invalid declarations at compile time, including missing
required names, duplicate suite hooks, duplicate case or test names, a suite
without cases, and a case without tests.

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

## Suite Arguments

Values that a suite needs from its environment, such as the address of a server under test, are
supplied as `KEY=VALUE` pairs and are readable everywhere through the `args` binding:

```rust
use rut::{TestResult, suite};

suite! {
    typename = ApiSuite;
    name = "API";

    setup {
        // Available in suite setup, before any test runs
        println!("targeting {}", args.get_or("ip", "127.0.0.1"));
    }

    test_case(name = "connection") {
        test(name = "reaches the server") {
            let ip = args.get_or("ip", "127.0.0.1");
            let port: u16 = args.parsed("port").unwrap().unwrap_or(80);

            if ping(ip, port).await {
                TestResult::passed()
            } else {
                TestResult::failed(format!("{ip}:{port} is unreachable"))
            }
        }
    }
}
```

`args` offers `get`, `get_or`, `contains`, `parsed::<T>` for a typed optional value, `required::<T>`
for a typed mandatory one, and `iter`. It is available in suite setup and teardown, case setup and
teardown, and test bodies, alongside `context` when the suite declares one.

With `cargo rut`, arguments are supplied after `--suite-args`, which must be the last option:

```console
cargo rut run path/to/suite.rs --suite-args ip=10.0.0.1 port=8080
```

An argument that is not in `KEY=VALUE` form stops the run. In manual runs, the same values are set
on the runner:

```rust
let report = SequentialRunner::new()
    .with_suite(Box::new(ApiSuite::new()))
    .with_suite_arg("ip", "10.0.0.1")
    .with_suite_arg("port", "8080")
    .run()
    .await?;
```

Use `with_suite_args` to supply a whole `SuiteArgs` at once. Reporters receive the same values
through `TestReporter::set_suite_args`.

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

## Custom Runners and Reporters

Runners and reporters are ordinary traits, so you can write your own and use them from a `main`
function without involving `cargo rut`.

A **runner** implements `TestRunner`. The example below wraps `SequentialRunner` and announces how
many passes it was asked to make:

```rust
use rut::{ReporterResult, SequentialRunner, SuiteReport, TestReporter, TestRunner, TestSuite};

pub struct RepeatRunner(SequentialRunner);

impl RepeatRunner {
    pub fn new(passes: usize) -> Self {
        println!("repeat runner: {passes} pass(es)");
        Self(SequentialRunner::new())
    }
}

#[async_trait::async_trait]
impl TestRunner for RepeatRunner {
    fn set_suite(&mut self, suite: Box<dyn TestSuite>) {
        self.0.set_suite(suite);
    }

    fn set_reporter(&mut self, reporter: Box<dyn TestReporter>) {
        self.0.set_reporter(reporter);
    }

    async fn run(self) -> ReporterResult<SuiteReport> {
        self.0.run().await
    }
}
```

`with_suite` and `with_reporter` come with the trait, so a custom runner is used exactly like a
built-in one:

```rust
let report = RepeatRunner::new(2)
    .with_suite(Box::new(CalculatorSuite::new()))
    .with_reporter(Box::new(StdoutReporter::new()))
    .run()
    .await?;
```

A **reporter** implements `TestReporter`. It receives the lifecycle events and owns the
`SuiteReport` it builds:

```rust
use rut::{ReporterResult, SuiteReport, TestReporter, TestResult, TestStatus};
use std::time::Duration;

pub struct SummaryReporter {
    report: SuiteReport,
    label: String,
}

#[async_trait::async_trait]
impl TestReporter for SummaryReporter {
    async fn report_start(
        &mut self,
        suite_name: &str,
        _test_cases: &[rut::TestCaseInfo],
        _started_at: chrono::DateTime<chrono::Utc>,
    ) -> ReporterResult<()> {
        self.report.suite_name = suite_name.to_string();
        Ok(())
    }

    async fn report_result(&mut self, _case: &str, result: &TestResult) -> ReporterResult<()> {
        match result.status {
            TestStatus::Passed => self.report.total_passed += 1,
            TestStatus::Skipped => self.report.total_skipped += 1,
            _ => self.report.total_failed += 1,
        }
        Ok(())
    }

    async fn report_finish(
        &mut self,
        _duration: Duration,
        _total_duration: Duration,
        _finished_at: chrono::DateTime<chrono::Utc>,
    ) -> ReporterResult<()> {
        println!("{}: {} passed", self.label, self.report.total_passed);
        Ok(())
    }

    fn get_report(&self) -> &SuiteReport {
        &self.report
    }

    // report_case_start, report_test_start and report_case_finish are also
    // required; this reporter ignores them and returns Ok(()).
}
```

Pass it to any runner with `with_reporter`, or combine it with others through `MultiReporter`.

## Runners and Reporters as Plugins

A custom runner or reporter becomes usable from `cargo rut` by declaring the command line options
it accepts. `cargo rut` asks every runner and reporter for its options, merges them into one command
line, and forwards the arguments to your test binary, so a plugin's options show up in
`cargo rut run --help` without any change to `cargo rut` itself.

Take the `RepeatRunner` above and declare its name and its `--repeat-count` option:

```rust
use rut::cli::{ArgSpec, CoreArgs, PluginArgs, RunnerPlugin};

impl RunnerPlugin for RepeatRunner {
    // Selected with `--runner repeat`
    const NAME: &'static str = "repeat";
    const ABOUT: &'static str = "Runs the suite sequentially, announcing a repeat count";

    fn args() -> Vec<ArgSpec> {
        vec![ArgSpec::value("repeat-count").value_name("N").default("1")]
    }

    fn from_args(args: &PluginArgs<'_>, core: &CoreArgs) -> anyhow::Result<Self> {
        let mut runner = RepeatRunner::new(args.parsed("repeat-count")?.unwrap_or(1));
        runner.apply(core);
        Ok(runner)
    }
}
```

`from_args` builds the runner from its own options plus `CoreArgs`, which carries the
runner-agnostic `--filter` and `--fail-fast` values.

`ReporterPlugin` mirrors it, except that `from_args` receives a `SuiteContext` describing the suite
being run. Reporters that write files use it to derive their output path, and `is_active` lets a
reporter run as soon as its own option is given, without naming it in `--reporters`:

```rust
use rut::cli::{ArgSpec, PluginArgs, ReporterPlugin, SuiteContext};

impl ReporterPlugin for SummaryReporter {
    // Selected with `--reporters summary`
    const NAME: &'static str = "summary";
    const ABOUT: &'static str = "Prints a single summary line";

    fn args() -> Vec<ArgSpec> {
        vec![ArgSpec::value("summary-label").value_name("TEXT").default("summary")]
    }

    fn from_args(args: &PluginArgs<'_>, _suite: &SuiteContext) -> anyhow::Result<Self> {
        Ok(SummaryReporter::new(args.value("summary-label").unwrap_or("summary")))
    }
}
```

Finally, list what the crate exports:

```rust
rut::export_plugins! {
    runners: [RepeatRunner],
    reporters: [SummaryReporter],
}
```

Point `cargo rut` at the crate and the new options are available:

```console
cargo rut run path/to/suite.rs --plugin-crate path/to/plugins --runner repeat --repeat-count 2
cargo rut run path/to/suite.rs --plugins-dir path/to/plugin-crates
```

`--plugin-crate` takes one crate directory, `--plugins-dir` a directory whose child crates are all
registered. Both are repeatable.

The plugins and the suites that use them may live in the same crate; point `--plugin-crate` at the
crate that owns the suite file:

```console
cargo rut run suites/colocated.rs --plugin-crate . --runner repeat
```

Two requirements for a plugin crate: it must declare `rut = { ..., features = ["cli"] }`, and
`rut::export_plugins!` must be invoked at its root.
[`rut/tests/fixtures/plugin`](rut/tests/fixtures/plugin) is a complete example that also holds a
suite.

Option names must not collide: the run stops with an error when two plugins declare the same option,
or when a plugin redeclares `--runner`, `--reporters`, `--filter`, or `--fail-fast`.

`--typename`, `--plugin-crate`, and `--plugins-dir` are the only options `cargo rut run` consumes
itself; everything else is forwarded. Use `--` when a forwarded option shares a name with one of
those three.

More complete examples are available in [`rut/examples/declarative.rs`](rut/examples/declarative.rs), [`rut/examples/sequential.rs`](rut/examples/sequential.rs), and [`rut/examples/parallel.rs`](rut/examples/parallel.rs).
