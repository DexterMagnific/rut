//! `rut` is an asynchronous Rust testing framework organized around suites,
//! cases, and tests.
//!
//! Most users define suites with the [`suite!`] macro, return [`TestResult`]
//! values from test bodies, and run the generated suite with one of the
//! built-in runners. The runtime also exposes traits for implementing custom
//! suites, cases, tests, runners, and reporters.
//!
//! A suite runs its suite setup once, then executes its selected cases. Each
//! case runs its setup, tests in declaration order, and teardown. Suite
//! teardown runs after all cases have finished. Test and lifecycle methods are
//! asynchronous, so setup, teardown, and test bodies may use `.await`.
//!
//! # Typical usage
//!
//! ```rust
//! use rut::suite;
//!
//! suite! {
//!     typename = ExampleSuite;
//!     name = "Example";
//!
//!     test_case(name = "basic behavior") {
//!         test(name = "passes") {
//!             rut::TestResult::passed()
//!         }
//!     }
//! }
//! ```
//!
//! See the [`suite!`] macro documentation for the complete declarative
//! syntax and the [`TestRunner`] trait for custom execution integrations.

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
    GTestReporter, JUnitReporter, MultiReporter, ReporterResult, StdoutReporter, TestReporter,
    TestReporterInternal,
};
pub use runner::{ParallelRunner, ParallelRunnerBuilder, SequentialRunner, TestRunner};
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
