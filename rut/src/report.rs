use chrono::{DateTime, Utc};
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The outcome of a test or case.
///
/// `Failed` and `TimedOut` count as failures. `Unstable` means the test first
/// failed and later passed after retries; it is not counted as a failure.
pub enum TestStatus {
    NotYetRun,
    Running,
    Passed,
    Failed,
    Skipped,
    TimedOut,
    Unstable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A source position associated with a test or failure.
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
/// The completed report for one suite execution.
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
/// Results and aggregate counts for one test case.
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
/// The final outcome and metadata for one test execution.
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub message: Option<String>,
    pub source: Option<SourceLocation>,
    pub failure_location: Option<SourceLocation>,
    pub duration: Duration,
    pub total_duration: Duration,
    pub properties: Vec<(String, String)>,
    pub failed_attempts: u32,
}

impl TestResult {
    /// Creates a passing result.
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
            failed_attempts: 0,
        }
    }

    /// Creates a result for a test that passed after failed retry attempts.
    pub fn unstable(failed_attempts: u32) -> Self {
        let mut result = Self::passed();
        result.status = TestStatus::Unstable;
        result.message = Some(format!(
            "test passed after {} failed attempt{}",
            failed_attempts,
            if failed_attempts == 1 { "" } else { "s" }
        ));
        result.failed_attempts = failed_attempts;
        result
    }

    #[track_caller]
    /// Creates a failed result and records the caller as its failure location.
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
            failed_attempts: 0,
        }
    }

    /// Creates a skipped result with a human-readable reason.
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
            failed_attempts: 0,
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
            failed_attempts: 0,
        }
    }

    /// Adds one property to this result.
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push((key.into(), value.into()));
        self
    }

    /// Adds several properties to this result.
    pub fn with_properties(mut self, props: Vec<(String, String)>) -> Self {
        self.properties.extend(props);
        self
    }
}

/// Type-erased, shareable context available to suite, case, and test code.
#[derive(Clone)]
pub struct TestContext {
    inner: Arc<dyn Any + Send + Sync>,
}

impl TestContext {
    /// Stores a context value for later typed access.
    pub fn new<T: Any + Send + Sync + 'static>(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }

    /// Returns the stored value when it has the requested concrete type.
    pub fn downcast_ref<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }
}

/// Key/value arguments supplied to a suite from outside, such as the address of
/// a server the tests should connect to.
///
/// Unlike [`TestContext`], these are provided before the suite runs, so they are
/// readable from suite setup as well as from case hooks and test bodies.
/// Cloning is a refcount bump, which is what lets the parallel runner hand a
/// copy to every spawned case.
#[derive(Clone, Debug, Default)]
pub struct SuiteArgs {
    values: Arc<std::collections::BTreeMap<String, String>>,
}

impl SuiteArgs {
    /// Creates an empty set of arguments.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a shared empty set, used as the default for runners without args.
    pub fn empty() -> &'static SuiteArgs {
        static EMPTY: std::sync::OnceLock<SuiteArgs> = std::sync::OnceLock::new();
        EMPTY.get_or_init(SuiteArgs::default)
    }

    /// Adds or replaces one argument.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        Arc::make_mut(&mut self.values).insert(key.into(), value.into());
    }

    /// Parses `KEY=VALUE` entries, rejecting those without a separator or key.
    pub fn parse<I, S>(pairs: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        pairs
            .into_iter()
            .map(|pair| parse_suite_arg(pair.as_ref()))
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|pairs| pairs.into_iter().collect())
    }

    /// Returns the raw value of an argument.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Returns the value of an argument, or `default` when it is absent.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    /// Returns whether an argument was supplied.
    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Parses the value of an argument into the requested type.
    pub fn parsed<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        match self.get(key) {
            None => Ok(None),
            Some(raw) => raw.parse::<T>().map(Some).map_err(|error| {
                anyhow::anyhow!("invalid value for suite argument '{key}': {error}")
            }),
        }
    }

    /// Parses the value of an argument, requiring it to be present.
    pub fn required<T>(&self, key: &str) -> anyhow::Result<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        self.parsed(key)?
            .ok_or_else(|| anyhow::anyhow!("missing suite argument '{key}'"))
    }

    /// Iterates over every argument in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

fn parse_suite_arg(pair: &str) -> anyhow::Result<(String, String)> {
    let (key, value) = pair
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("suite argument '{pair}' is not in KEY=VALUE form"))?;
    let key = key.trim();

    if key.is_empty() {
        return Err(anyhow::anyhow!("suite argument '{pair}' has an empty key"));
    }

    Ok((key.to_string(), value.to_string()))
}

impl<K, V> FromIterator<(K, V)> for SuiteArgs
where
    K: Into<String>,
    V: Into<String>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(pairs: I) -> Self {
        Self {
            values: Arc::new(
                pairs
                    .into_iter()
                    .map(|(key, value)| (key.into(), value.into()))
                    .collect(),
            ),
        }
    }
}

/// Static suite and case information used to initialize a report.
#[derive(Debug, Clone)]
pub struct TestCaseInfo {
    pub name: String,
    pub tests: Vec<TestInfo>,
}

#[derive(Debug, Clone)]
/// Static information about one declared test.
pub struct TestInfo {
    pub name: String,
    pub source: Option<SourceLocation>,
}

impl SuiteReport {
    /// Creates a report pre-populated with the declared cases and tests.
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
                        failed_attempts: 0,
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
