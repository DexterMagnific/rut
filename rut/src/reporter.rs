use async_trait::async_trait;
use std::time::Duration;

use crate::report::{BoxFuture, SuiteReport, TestCaseInfo, TestResult};
pub use crate::reporters::StdoutReporter;

pub trait TestReporterInternal: Send + Sync {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
    ) -> BoxFuture<'a, ()>;
    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
    ) -> BoxFuture<'a, ()>;
    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ()>;
    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ()>;
    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()>;
    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()>;
    fn get_report(&self) -> &SuiteReport;
}

/// User-facing trait - implement this in your test code for custom reporters
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait TestReporter: Send + Sync {
    async fn report_start(&mut self, suite_name: &str, test_cases: &[TestCaseInfo]);
    async fn report_case_start(&mut self, case_name: &str, test_count: usize);
    async fn report_test_start(&mut self, case_name: &str, test_name: &str);
    async fn report_result(&mut self, case_name: &str, result: &TestResult);
    async fn report_case_finish(
        &mut self,
        case_name: &str,
        duration: Duration,
        total_duration: Duration,
    );
    async fn report_finish(&mut self, duration: Duration, total_duration: Duration);
    fn get_report(&self) -> &SuiteReport;
}

/// Blanket implementation: converts external TestReporter to internal TestReporterInternal
impl<T: TestReporter + ?Sized> TestReporterInternal for T {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.report_start(suite_name, test_cases).await })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.report_case_start(case_name, test_count).await })
    }

    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.report_test_start(case_name, test_name).await })
    }

    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.report_result(case_name, result).await })
    }

    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            self.report_case_finish(case_name, duration, total_duration)
                .await
        })
    }

    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.report_finish(duration, total_duration).await })
    }

    fn get_report(&self) -> &SuiteReport {
        TestReporter::get_report(self)
    }
}

// MultiReporter - forwards calls to multiple sub-reporters
pub struct MultiReporter {
    reporters: Vec<Box<dyn TestReporterInternal>>,
}

impl MultiReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter(mut self, reporter: Box<dyn TestReporterInternal>) -> Self {
        self.reporters.push(reporter);
        self
    }

    pub fn with_reporters(mut self, reporters: Vec<Box<dyn TestReporterInternal>>) -> Self {
        self.reporters = reporters;
        self
    }
}

impl Default for MultiReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl TestReporterInternal for MultiReporter {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_start(suite_name, test_cases).await;
            }
        })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_case_start(case_name, test_count).await;
            }
        })
    }

    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_test_start(case_name, test_name).await;
            }
        })
    }

    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_result(case_name, result).await;
            }
        })
    }

    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter
                    .report_case_finish(case_name, duration, total_duration)
                    .await;
            }
        })
    }

    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_finish(duration, total_duration).await;
            }
        })
    }

    fn get_report(&self) -> &SuiteReport {
        self.reporters
            .first()
            .map(|r| r.get_report())
            .expect("MultiReporter has no sub-reporters")
    }
}

// StdoutReporter is re-exported at the top of this module
