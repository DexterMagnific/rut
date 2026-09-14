use chrono::{DateTime, Utc};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    NotYetRun,
    Running,
    Passed,
    Failed,
    Skipped,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SuiteReport {
    pub suite_name: String,
    pub test_cases: Vec<CaseReport>,
    pub total_passed: usize,
    pub total_failed: usize,
    pub total_skipped: usize,
    pub duration: Duration,
    pub total_duration: Duration,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CaseReport {
    pub name: String,
    pub tests: Vec<TestResult>,
    pub status: TestStatus,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration: Duration,
    pub total_duration: Duration,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub message: Option<String>,
    pub source: Option<SourceLocation>,
    pub failure_location: Option<SourceLocation>,
    pub duration: Duration,
    pub total_duration: Duration,
    pub properties: Vec<(String, String)>,
}

impl TestResult {
    pub fn passed() -> Self {
        Self {
            name: String::new(),
            status: TestStatus::Passed,
            message: None,
            source: None,
            failure_location: None,
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            properties: Vec::new(),
        }
    }

    #[track_caller]
    pub fn failed(message: impl Into<String>) -> Self {
        let caller = std::panic::Location::caller();
        Self {
            name: String::new(),
            status: TestStatus::Failed,
            message: Some(message.into()),
            source: None,
            failure_location: Some(SourceLocation::new(
                caller.file(),
                caller.line(),
                caller.column(),
            )),
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            properties: Vec::new(),
        }
    }

    pub fn skipped(reason: impl Into<String>) -> Self {
        Self {
            name: String::new(),
            status: TestStatus::Skipped,
            message: Some(reason.into()),
            source: None,
            failure_location: None,
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            properties: Vec::new(),
        }
    }

    pub(crate) fn timed_out(timeout: Duration) -> Self {
        Self {
            name: String::new(),
            status: TestStatus::TimedOut,
            message: Some(format!("test timed out after {timeout:?}")),
            source: None,
            failure_location: None,
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            properties: Vec::new(),
        }
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push((key.into(), value.into()));
        self
    }

    pub fn with_properties(mut self, props: Vec<(String, String)>) -> Self {
        self.properties.extend(props);
        self
    }
}

/// Type-erased context that can hold any data
#[derive(Clone)]
pub struct TestContext {
    inner: Arc<dyn Any + Send + Sync>,
}

impl TestContext {
    pub fn new<T: Any + Send + Sync + 'static>(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }

    pub fn downcast_ref<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }
}

/// Information needed to create a pre-populated suite report
#[derive(Debug, Clone)]
pub struct TestCaseInfo {
    pub name: String,
    pub tests: Vec<TestInfo>,
}

#[derive(Debug, Clone)]
pub struct TestInfo {
    pub name: String,
    pub source: Option<SourceLocation>,
}

impl SuiteReport {
    pub fn new(suite_name: &str, test_cases: &[TestCaseInfo], started_at: DateTime<Utc>) -> Self {
        let test_cases_vec: Vec<CaseReport> = test_cases
            .iter()
            .map(|c| {
                let entries: Vec<TestResult> = c
                    .tests
                    .iter()
                    .map(|test| TestResult {
                        name: test.name.clone(),
                        status: TestStatus::NotYetRun,
                        message: None,
                        source: test.source.clone(),
                        failure_location: None,
                        duration: Duration::ZERO,
                        total_duration: Duration::ZERO,
                        properties: Vec::new(),
                    })
                    .collect();

                CaseReport {
                    name: c.name.clone(),
                    tests: entries,
                    status: TestStatus::NotYetRun,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    duration: Duration::ZERO,
                    total_duration: Duration::ZERO,
                    started_at: None,
                    finished_at: None,
                }
            })
            .collect();

        Self {
            suite_name: suite_name.to_string(),
            test_cases: test_cases_vec,
            total_passed: 0,
            total_failed: 0,
            total_skipped: 0,
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            started_at,
            finished_at: started_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_captures_its_call_site() {
        let expected_line = line!() + 1;
        let result = TestResult::failed("failure");

        let location = result.failure_location.unwrap();
        assert_eq!(location.file, file!());
        assert_eq!(location.line, expected_line);
        assert!(location.column > 0);
    }

    #[test]
    fn suite_report_preserves_declared_test_source() {
        let source = SourceLocation::new("tests/example.rs", 12, 9);
        let report = SuiteReport::new(
            "suite",
            &[TestCaseInfo {
                name: "case".to_string(),
                tests: vec![TestInfo {
                    name: "test".to_string(),
                    source: Some(source.clone()),
                }],
            }],
            Utc::now(),
        );

        assert_eq!(report.test_cases[0].tests[0].source, Some(source));
        assert_eq!(report.test_cases[0].tests[0].failure_location, None);
    }
}
