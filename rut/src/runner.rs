use crate::case::TestCaseInternal;
use crate::report::{BoxFuture, SuiteReport, TestResult};
use std::time::Duration;

pub type CaseData = (String, Vec<String>, Box<dyn TestCaseInternal>);

pub struct CaseResult {
    pub name: String,
    pub test_names: Vec<String>,
    pub results: Vec<(String, TestResult)>,
    pub duration: Duration,
    pub total_duration: Duration,
}

pub trait TestRunnerInternal: Send + Sync {
    fn with_suite(self, suite: Box<dyn crate::suite::TestSuiteInternal>) -> Self
    where
        Self: Sized;
    fn with_reporter(self, reporter: Box<dyn crate::reporter::TestReporterInternal>) -> Self
    where
        Self: Sized;
    fn run(self) -> BoxFuture<'static, SuiteReport>;
}

// Re-export runners from the crate-level runners module
pub use crate::runners::{ParallelRunner, ParallelRunnerBuilder, SequentialRunner};
