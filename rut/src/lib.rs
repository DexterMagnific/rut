extern crate self as rut;

mod panic_capture;

pub mod case;
pub mod report;
pub mod reporter;
pub mod reporters;
pub mod runner;
pub mod runners;
pub mod suite;
pub mod test;

pub use case::{TestCase, TestCaseInternal};
pub use report::{
    BoxFuture, CaseReport, SourceLocation, SuiteReport, TestCaseInfo, TestContext, TestInfo,
    TestResult, TestStatus,
};
pub use reporter::{
    GTestReporter, JUnitReporter, MultiReporter, ReporterError, ReporterResult, StdoutReporter,
    TestReporter, TestReporterInternal,
};
pub use runner::{ParallelRunner, ParallelRunnerBuilder, SequentialRunner, TestRunnerInternal};
pub use rut_macros::suite;
pub use suite::{TestSuite, TestSuiteBuilder, TestSuiteInternal};
pub use test::{Test, TestInternal};

#[doc(hidden)]
pub mod __private {
    pub use async_trait::async_trait;

    pub struct ContextInitializer<T> {
        value: Option<T>,
    }

    impl<T> Default for ContextInitializer<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> ContextInitializer<T> {
        pub fn new() -> Self {
            Self { value: None }
        }

        pub fn set(&mut self, value: T) {
            self.value = Some(value);
        }

        pub fn into_inner(self) -> Option<T> {
            self.value
        }
    }
}
