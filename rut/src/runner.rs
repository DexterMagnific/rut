use crate::report::SuiteReport;
use crate::reporter::{ReporterResult, TestReporter};
use crate::suite::TestSuite;
use crate::{SourceLocation, Test, TestContext, TestResult};
use async_trait::async_trait;
use std::future::Future;

/// Details captured when a test future panics while it is being polled.
///
/// The panic payload is converted to a message, and the panic hook records a
/// source location and backtrace when available. Panics from unrelated code
/// are forwarded to the hook that was installed before `rut` began capturing.
#[derive(Debug)]
pub struct PanicInfo {
    message: String,
    location: Option<SourceLocation>,
    backtrace: String,
}

impl PanicInfo {
    pub(crate) fn new(
        message: String,
        location: Option<SourceLocation>,
        backtrace: String,
    ) -> Self {
        Self {
            message,
            location,
            backtrace,
        }
    }

    /// Returns the panic payload as a readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source location reported by the panic hook, when available.
    pub fn location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }

    /// Returns the captured backtrace text.
    pub fn backtrace(&self) -> &str {
        &self.backtrace
    }
}

/// Polls an asynchronous future while converting panics into [`PanicInfo`].
///
/// Panic capture is cooperative: a panic is caught when it occurs during a
/// poll of the future. A future that never yields can still block its executor
/// thread and cannot be interrupted by this helper.
pub async fn catch_test_panic<F>(future: F) -> Result<F::Output, PanicInfo>
where
    F: Future,
{
    crate::panic_capture::catch_test_panic(future).await
}

/// Runs one test, catching panics and enforcing its declared timeout, if any.
///
/// Timeouts are cooperative and only take effect when the test future yields.
/// This helper does not apply the test's retry policy; use
/// [`run_test_with_retries`] when retry handling is required.
pub async fn run_test_with_timeout(
    test: &dyn Test,
    ctx: Option<&TestContext>,
) -> TestResult {
    crate::panic_capture::run_test_with_timeout(test, ctx).await
}

/// Runs one test with panic capture, timeout handling, and declared retries.
///
/// The retry count is the number of additional attempts after the initial
/// execution. A test that eventually passes is returned as `Unstable` and
/// records its failed attempts; a test that exhausts its retries retains its
/// final failure status.
pub async fn run_test_with_retries(
    test: &dyn Test,
    ctx: Option<&TestContext>,
) -> TestResult {
    crate::panic_capture::run_test_with_retries(test, ctx).await
}

#[async_trait]
/// Executes a suite and delivers lifecycle events to a reporter.
///
/// Configure an implementation with [`with_suite`](Self::with_suite) and
/// [`with_reporter`](Self::with_reporter), then call [`run`](Self::run).
/// [`crate::SequentialRunner`] and [`crate::ParallelRunner`] provide the
/// built-in execution strategies; implement this trait when integrating a
/// different scheduling policy.
pub trait TestRunner: Send + Sync {
    fn with_suite(self, suite: Box<dyn TestSuite>) -> Self
    where
        Self: Sized;
    fn with_reporter(self, reporter: Box<dyn TestReporter>) -> Self
    where
        Self: Sized;
    async fn run(self) -> ReporterResult<SuiteReport>
    where
        Self: Sized;
}

// Re-export runners from the crate-level runners module
pub use crate::runners::{ParallelRunner, ParallelRunnerBuilder, SequentialRunner};
