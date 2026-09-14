use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::report::{BoxFuture, SuiteReport, TestCaseInfo, TestResult};
pub use crate::reporters::{GTestReporter, JUnitReporter, StdoutReporter};

pub type ReporterResult<T> = anyhow::Result<T>;

pub trait TestReporterInternal: Send + Sync {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>>;
    fn get_report(&self) -> &SuiteReport;
}

/// User-facing trait - implement this in your test code for custom reporters
/// Uses async_trait for clean async fn syntax
#[async_trait]
pub trait TestReporter: Send + Sync {
    async fn report_start(
        &mut self,
        suite_name: &str,
        test_cases: &[TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()>;
    async fn report_case_start(
        &mut self,
        case_name: &str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()>;
    async fn report_test_start(&mut self, case_name: &str, test_name: &str) -> ReporterResult<()>;
    async fn report_result(&mut self, case_name: &str, result: &TestResult) -> ReporterResult<()>;
    async fn report_case_finish(
        &mut self,
        case_name: &str,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> ReporterResult<()>;
    async fn report_finish(
        &mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> ReporterResult<()>;
    fn get_report(&self) -> &SuiteReport;
}

/// Blanket implementation: converts external TestReporter to internal TestReporterInternal
impl<T: TestReporter + ?Sized> TestReporterInternal for T {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move { self.report_start(suite_name, test_cases, started_at).await })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            self.report_case_start(case_name, test_count, started_at)
                .await
        })
    }

    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move { self.report_test_start(case_name, test_name).await })
    }

    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move { self.report_result(case_name, result).await })
    }

    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            self.report_case_finish(case_name, duration, total_duration, finished_at)
                .await
        })
    }

    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            self.report_finish(duration, total_duration, finished_at)
                .await
        })
    }

    fn get_report(&self) -> &SuiteReport {
        TestReporter::get_report(self)
    }
}

// MultiReporter - forwards calls to multiple sub-reporters
pub struct MultiReporter {
    reporters: Vec<Box<dyn TestReporter>>,
}

impl MultiReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter(mut self, reporter: Box<dyn TestReporter>) -> Self {
        self.reporters.push(reporter);
        self
    }

    pub fn with_reporters(mut self, reporters: Vec<Box<dyn TestReporter>>) -> Self {
        self.reporters = reporters;
        self
    }
}

impl Default for MultiReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestReporter for MultiReporter {
    async fn report_start(
        &mut self,
        suite_name: &str,
        test_cases: &[TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter
                .report_start(suite_name, test_cases, started_at)
                .await?;
        }
        Ok(())
    }

    async fn report_case_start(
        &mut self,
        case_name: &str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter
                .report_case_start(case_name, test_count, started_at)
                .await?;
        }
        Ok(())
    }

    async fn report_test_start(
        &mut self,
        case_name: &str,
        test_name: &str,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter.report_test_start(case_name, test_name).await?;
        }
        Ok(())
    }

    async fn report_result(
        &mut self,
        case_name: &str,
        result: &TestResult,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter.report_result(case_name, result).await?;
        }
        Ok(())
    }

    async fn report_case_finish(
        &mut self,
        case_name: &str,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter
                .report_case_finish(case_name, duration, total_duration, finished_at)
                .await?;
        }
        Ok(())
    }

    async fn report_finish(
        &mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        for reporter in &mut self.reporters {
            reporter
                .report_finish(duration, total_duration, finished_at)
                .await?;
        }
        Ok(())
    }

    fn get_report(&self) -> &SuiteReport {
        self.reporters
            .first()
            .map(|r| r.get_report())
            .expect("MultiReporter has no sub-reporters")
    }
}

// StdoutReporter is re-exported at the top of this module

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::{GTestReporter, JUnitReporter};
    use std::fmt;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct CustomReporterError;

    impl fmt::Display for CustomReporterError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("custom reporter failed")
        }
    }

    impl std::error::Error for CustomReporterError {}

    struct FailingCustomReporter {
        report: Option<SuiteReport>,
    }

    #[async_trait]
    impl TestReporter for FailingCustomReporter {
        async fn report_start(
            &mut self,
            suite_name: &str,
            test_cases: &[TestCaseInfo],
            started_at: DateTime<Utc>,
        ) -> ReporterResult<()> {
            self.report = Some(SuiteReport::new(suite_name, test_cases, started_at));
            Ok(())
        }

        async fn report_case_start(
            &mut self,
            _case_name: &str,
            _test_count: usize,
            _started_at: DateTime<Utc>,
        ) -> ReporterResult<()> {
            Ok(())
        }

        async fn report_test_start(
            &mut self,
            _case_name: &str,
            _test_name: &str,
        ) -> ReporterResult<()> {
            Ok(())
        }

        async fn report_result(
            &mut self,
            _case_name: &str,
            _result: &TestResult,
        ) -> ReporterResult<()> {
            Ok(())
        }

        async fn report_case_finish(
            &mut self,
            _case_name: &str,
            _duration: Duration,
            _total_duration: Duration,
            _finished_at: DateTime<Utc>,
        ) -> ReporterResult<()> {
            Ok(())
        }

        async fn report_finish(
            &mut self,
            _duration: Duration,
            _total_duration: Duration,
            _finished_at: DateTime<Utc>,
        ) -> ReporterResult<()> {
            Err(CustomReporterError.into())
        }

        fn get_report(&self) -> &SuiteReport {
            self.report.as_ref().expect("report_start not called")
        }
    }

    #[tokio::test]
    async fn multi_reporter_propagates_junit_write_failures() {
        let directory = TempDir::new().unwrap();
        let blocking_file = directory.path().join("not-a-directory");
        std::fs::write(&blocking_file, "content").unwrap();
        let mut reporter = MultiReporter::new()
            .add_reporter(Box::new(StdoutReporter::new()))
            .add_reporter(Box::new(JUnitReporter::new(
                blocking_file.join("report.xml"),
            )));

        let started_at = Utc::now();
        let finished_at = started_at + chrono::TimeDelta::seconds(1);
        TestReporter::report_start(&mut reporter, "suite", &[], started_at)
            .await
            .unwrap();
        let error = TestReporter::report_finish(
            &mut reporter,
            Duration::ZERO,
            Duration::ZERO,
            finished_at,
        )
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to create report directory")
        );
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(TestReporter::get_report(&reporter).suite_name, "suite");
        assert_eq!(TestReporter::get_report(&reporter).started_at, started_at);
        assert_eq!(TestReporter::get_report(&reporter).finished_at, finished_at);
    }

    #[tokio::test]
    async fn multi_reporter_propagates_gtest_write_failures() {
        let directory = TempDir::new().unwrap();
        let blocking_file = directory.path().join("not-a-directory");
        std::fs::write(&blocking_file, "content").unwrap();
        let mut reporter = MultiReporter::new()
            .add_reporter(Box::new(StdoutReporter::new()))
            .add_reporter(Box::new(GTestReporter::new(
                blocking_file.join("report.json"),
            )));

        let started_at = Utc::now();
        let finished_at = started_at + chrono::TimeDelta::seconds(1);
        TestReporter::report_start(&mut reporter, "suite", &[], started_at)
            .await
            .unwrap();
        let error = TestReporter::report_finish(
            &mut reporter,
            Duration::ZERO,
            Duration::ZERO,
            finished_at,
        )
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to create report directory")
        );
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(TestReporter::get_report(&reporter).suite_name, "suite");
        assert_eq!(TestReporter::get_report(&reporter).finished_at, finished_at);
    }

    #[tokio::test]
    async fn custom_reporter_errors_remain_downcastable() {
        let mut reporter = MultiReporter::new()
            .add_reporter(Box::new(StdoutReporter::new()))
            .add_reporter(Box::new(FailingCustomReporter { report: None }));
        let started_at = Utc::now();
        TestReporter::report_start(&mut reporter, "suite", &[], started_at)
            .await
            .unwrap();

        let error = TestReporter::report_finish(
            &mut reporter,
            Duration::ZERO,
            Duration::ZERO,
            started_at,
        )
            .await
            .unwrap_err();

        assert!(error.downcast_ref::<CustomReporterError>().is_some());
        assert_eq!(error.to_string(), "custom reporter failed");
        assert_eq!(TestReporter::get_report(&reporter).suite_name, "suite");
    }
}
