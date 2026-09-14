use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::report::{BoxFuture, SuiteReport, TestCaseInfo, TestResult};
pub use crate::reporters::{JUnitReporter, StdoutReporter};

#[derive(Debug, thiserror::Error)]
pub enum ReporterError {
    #[error("failed to write JUnit report to {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize JUnit report: {0}")]
    Xml(String),
}

pub type ReporterResult<T> = std::result::Result<T, ReporterError>;

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
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter
                    .report_start(suite_name, test_cases, started_at)
                    .await?;
            }
            Ok(())
        })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter
                    .report_case_start(case_name, test_count, started_at)
                    .await?;
            }
            Ok(())
        })
    }

    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_test_start(case_name, test_name).await?;
            }
            Ok(())
        })
    }

    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter.report_result(case_name, result).await?;
            }
            Ok(())
        })
    }

    fn report_case_finish<'a>(
        &'a mut self,
        case_name: &'a str,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter
                    .report_case_finish(case_name, duration, total_duration, finished_at)
                    .await?;
            }
            Ok(())
        })
    }

    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            for reporter in &mut self.reporters {
                reporter
                    .report_finish(duration, total_duration, finished_at)
                    .await?;
            }
            Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::JUnitReporter;
    use tempfile::TempDir;

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
        reporter
            .report_start("suite", &[], started_at)
            .await
            .unwrap();
        let error = reporter
            .report_finish(Duration::ZERO, Duration::ZERO, finished_at)
            .await
            .unwrap_err();

        assert!(matches!(error, ReporterError::Write { .. }));
        assert_eq!(reporter.get_report().suite_name, "suite");
        assert_eq!(reporter.get_report().started_at, started_at);
        assert_eq!(reporter.get_report().finished_at, finished_at);
    }
}
