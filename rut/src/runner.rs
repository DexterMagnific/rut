use crate::report::SuiteReport;
use crate::reporter::{ReporterResult, TestReporter};
use crate::suite::TestSuite;
use async_trait::async_trait;

#[async_trait]
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
