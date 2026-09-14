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
}

#[derive(Debug, Clone)]
pub struct SuiteReport {
    pub suite_name: String,
    pub test_cases: Vec<CaseReport>,
    pub total_passed: usize,
    pub total_failed: usize,
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
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            properties: Vec::new(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            name: String::new(),
            status: TestStatus::Failed,
            message: Some(message.into()),
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
    pub test_names: Vec<String>,
}

impl SuiteReport {
    pub fn new(suite_name: &str, test_cases: &[TestCaseInfo]) -> Self {
        let test_cases_vec: Vec<CaseReport> = test_cases
            .iter()
            .map(|c| {
                let entries: Vec<TestResult> = c
                    .test_names
                    .iter()
                    .map(|name| TestResult {
                        name: name.clone(),
                        status: TestStatus::NotYetRun,
                        message: None,
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
                    duration: Duration::ZERO,
                    total_duration: Duration::ZERO,
                    started_at: None,
                    finished_at: None,
                }
            })
            .collect();

        let now = Utc::now();
        Self {
            suite_name: suite_name.to_string(),
            test_cases: test_cases_vec,
            total_passed: 0,
            total_failed: 0,
            duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            started_at: now,
            finished_at: now,
        }
    }
}
