use crate::report::{BoxFuture, CaseReport, SuiteReport, TestCaseInfo, TestResult, TestStatus};
use crate::reporter::{ReporterError, ReporterResult, TestReporterInternal};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

pub struct GTestReporter {
    path: PathBuf,
    report: Option<SuiteReport>,
}

impl GTestReporter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            report: None,
        }
    }

    fn report(&self) -> &SuiteReport {
        self.report.as_ref().expect("report_start not called")
    }

    fn report_mut(&mut self) -> &mut SuiteReport {
        self.report.as_mut().expect("report_start not called")
    }

    fn write_report(&self) -> ReporterResult<()> {
        let json = serde_json::to_vec_pretty(&GTestRun::from(self.report()))
            .map_err(|error| ReporterError::Json(error.to_string()))?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| ReporterError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        std::fs::write(&self.path, json).map_err(|source| ReporterError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

impl TestReporterInternal for GTestReporter {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            self.report = Some(SuiteReport::new(suite_name, test_cases, started_at));
            Ok(())
        })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        _test_count: usize,
        started_at: DateTime<Utc>,
    ) -> BoxFuture<'a, ReporterResult<()>> {
        Box::pin(async move {
            if let Some(case) = self
                .report_mut()
                .test_cases
                .iter_mut()
                .find(|case| case.name == case_name)
            {
                case.status = TestStatus::Running;
                case.started_at = Some(started_at);
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
            if let Some(test) = self
                .report_mut()
                .test_cases
                .iter_mut()
                .find(|case| case.name == case_name)
                .and_then(|case| case.tests.iter_mut().find(|test| test.name == test_name))
            {
                test.status = TestStatus::Running;
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
            let report = self.report_mut();
            if let Some(case) = report
                .test_cases
                .iter_mut()
                .find(|case| case.name == case_name)
                && let Some(test) = case.tests.iter_mut().find(|test| test.name == result.name)
            {
                test.status = result.status;
                test.message = result.message.clone();
                test.duration = result.duration;
                test.total_duration = result.total_duration;
                test.properties = result.properties.clone();

                match result.status {
                    TestStatus::Passed => {
                        case.passed += 1;
                        report.total_passed += 1;
                    }
                    TestStatus::Failed => {
                        case.failed += 1;
                        report.total_failed += 1;
                    }
                    TestStatus::NotYetRun | TestStatus::Running => {}
                }
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
            if let Some(case) = self
                .report_mut()
                .test_cases
                .iter_mut()
                .find(|case| case.name == case_name)
            {
                case.status = if case.failed > 0 {
                    TestStatus::Failed
                } else {
                    TestStatus::Passed
                };
                case.finished_at = Some(finished_at);
                case.duration = duration;
                case.total_duration = total_duration;
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
            let report = self.report_mut();
            report.finished_at = finished_at;
            report.duration = duration;
            report.total_duration = total_duration;
            self.write_report()
        })
    }

    fn get_report(&self) -> &SuiteReport {
        self.report()
    }
}

#[derive(Serialize)]
struct GTestRun<'a> {
    tests: usize,
    failures: usize,
    disabled: usize,
    errors: usize,
    timestamp: String,
    time: String,
    name: &'a str,
    testsuites: Vec<GTestSuite<'a>>,
}

impl<'a> From<&'a SuiteReport> for GTestRun<'a> {
    fn from(report: &'a SuiteReport) -> Self {
        Self {
            tests: report.test_cases.iter().map(|case| case.tests.len()).sum(),
            failures: report.total_failed,
            disabled: 0,
            errors: 0,
            timestamp: timestamp(report.started_at),
            time: duration(report.total_duration),
            name: &report.suite_name,
            testsuites: report.test_cases.iter().map(GTestSuite::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct GTestSuite<'a> {
    name: &'a str,
    tests: usize,
    failures: usize,
    disabled: usize,
    errors: usize,
    time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    testsuite: Vec<GTestTest<'a>>,
}

impl<'a> From<&'a CaseReport> for GTestSuite<'a> {
    fn from(case: &'a CaseReport) -> Self {
        Self {
            name: &case.name,
            tests: case.tests.len(),
            failures: case.failed,
            disabled: 0,
            errors: 0,
            time: duration(case.total_duration),
            timestamp: case.started_at.map(timestamp),
            testsuite: case
                .tests
                .iter()
                .map(|test| GTestTest::new(test, &case.name))
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct GTestTest<'a> {
    name: &'a str,
    status: &'static str,
    result: &'static str,
    time: String,
    classname: &'a str,
    #[serde(flatten)]
    properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failures: Option<Vec<GTestFailure<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<Vec<GTestSkipped>>,
}

impl<'a> GTestTest<'a> {
    fn new(test: &'a TestResult, classname: &'a str) -> Self {
        let unfinished = matches!(test.status, TestStatus::NotYetRun | TestStatus::Running);
        let properties = test
            .properties
            .iter()
            .map(|(name, value)| (format!("prop_{name}"), value.clone()))
            .collect();
        let failures = (test.status == TestStatus::Failed).then(|| {
            vec![GTestFailure {
                failure: test.message.as_deref().unwrap_or("test failed"),
                failure_type: "",
            }]
        });
        let skipped = unfinished.then(|| {
            vec![GTestSkipped {
                message: "test did not run to completion",
            }]
        });

        Self {
            name: &test.name,
            status: "RUN",
            result: if unfinished { "SKIPPED" } else { "COMPLETED" },
            time: duration(test.total_duration),
            classname,
            properties,
            failures,
            skipped,
        }
    }
}

#[derive(Serialize)]
struct GTestFailure<'a> {
    failure: &'a str,
    #[serde(rename = "type")]
    failure_type: &'static str,
}

#[derive(Serialize)]
struct GTestSkipped {
    message: &'static str,
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn duration(value: Duration) -> String {
    let milliseconds = value.as_millis();
    let seconds = milliseconds / 1000;
    let remainder = milliseconds % 1000;
    if remainder == 0 {
        format!("{seconds}s")
    } else {
        let fraction = format!("{remainder:03}");
        format!("{seconds}.{}s", fraction.trim_end_matches('0'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::TempDir;

    fn at(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[tokio::test]
    async fn writes_google_test_json_with_failures_skips_and_properties() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("nested/report.json");
        let mut reporter = GTestReporter::new(&path);
        reporter
            .report_start(
                "Calculator",
                &[TestCaseInfo {
                    name: "arithmetic".to_string(),
                    test_names: vec![
                        "passes".to_string(),
                        "fails".to_string(),
                        "later".to_string(),
                    ],
                }],
                at("2026-09-08T10:00:00Z"),
            )
            .await
            .unwrap();
        reporter
            .report_case_start("arithmetic", 3, at("2026-09-08T10:00:01Z"))
            .await
            .unwrap();
        reporter
            .report_test_start("arithmetic", "passes")
            .await
            .unwrap();
        let mut passed = TestResult::passed()
            .with_property("category", "static")
            .with_property("category", "runtime")
            .with_property("status", "property");
        passed.name = "passes".to_string();
        passed.total_duration = Duration::from_millis(12);
        reporter.report_result("arithmetic", &passed).await.unwrap();
        reporter
            .report_test_start("arithmetic", "fails")
            .await
            .unwrap();
        let mut failed = TestResult::failed("expected \"four\"\nreceived five");
        failed.name = "fails".to_string();
        failed.total_duration = Duration::from_millis(1200);
        reporter.report_result("arithmetic", &failed).await.unwrap();
        reporter
            .report_case_finish(
                "arithmetic",
                Duration::from_millis(1212),
                Duration::from_millis(1234),
                at("2026-09-08T10:00:02Z"),
            )
            .await
            .unwrap();
        reporter
            .report_finish(
                Duration::from_millis(1234),
                Duration::from_millis(2345),
                at("2026-09-08T10:00:03Z"),
            )
            .await
            .unwrap();

        let json: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(json["name"], "Calculator");
        assert_eq!(json["tests"], 3);
        assert_eq!(json["failures"], 1);
        assert_eq!(json["disabled"], 0);
        assert_eq!(json["errors"], 0);
        assert_eq!(json["timestamp"], "2026-09-08T10:00:00Z");
        assert_eq!(json["time"], "2.345s");

        let suite = &json["testsuites"][0];
        assert_eq!(suite["name"], "arithmetic");
        assert_eq!(suite["time"], "1.234s");
        assert_eq!(suite["timestamp"], "2026-09-08T10:00:01Z");

        let tests = suite["testsuite"].as_array().unwrap();
        assert_eq!(tests[0]["status"], "RUN");
        assert_eq!(tests[0]["result"], "COMPLETED");
        assert_eq!(tests[0]["time"], "0.012s");
        assert_eq!(tests[0]["prop_category"], "runtime");
        assert_eq!(tests[0]["prop_status"], "property");
        assert_eq!(
            tests[1]["failures"][0]["failure"],
            "expected \"four\"\nreceived five"
        );
        assert_eq!(tests[1]["failures"][0]["type"], "");
        assert_eq!(tests[2]["result"], "SKIPPED");
        assert_eq!(
            tests[2]["skipped"][0]["message"],
            "test did not run to completion"
        );
        assert!(tests[0].get("file").is_none());
        assert!(tests[0].get("line").is_none());
        assert!(tests[0].get("timestamp").is_none());
    }

    #[tokio::test]
    async fn reports_an_invalid_destination() {
        let directory = TempDir::new().unwrap();
        let blocking_file = directory.path().join("not-a-directory");
        std::fs::write(&blocking_file, "content").unwrap();
        let path = blocking_file.join("report.json");
        let mut reporter = GTestReporter::new(&path);
        reporter
            .report_start("suite", &[], at("2026-09-08T10:00:00Z"))
            .await
            .unwrap();

        let error = reporter
            .report_finish(Duration::ZERO, Duration::ZERO, at("2026-09-08T10:00:01Z"))
            .await
            .unwrap_err();

        assert!(
            matches!(error, ReporterError::Write { path: error_path, .. } if error_path == path)
        );
    }

    #[tokio::test]
    async fn writes_empty_runs_and_overwrites_existing_files() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("report.json");
        std::fs::write(&path, "stale trailing content").unwrap();
        let mut reporter = GTestReporter::new(&path);
        reporter
            .report_start("empty suite", &[], at("2026-09-08T10:00:00Z"))
            .await
            .unwrap();
        reporter
            .report_finish(Duration::ZERO, Duration::ZERO, at("2026-09-08T10:00:01Z"))
            .await
            .unwrap();

        let contents = std::fs::read_to_string(path).unwrap();
        let json: Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["name"], "empty suite");
        assert_eq!(json["tests"], 0);
        assert_eq!(json["testsuites"], serde_json::json!([]));
        assert!(!contents.contains("stale trailing content"));
    }

    #[test]
    fn formats_durations_at_millisecond_precision() {
        assert_eq!(duration(Duration::ZERO), "0s");
        assert_eq!(duration(Duration::from_millis(3)), "0.003s");
        assert_eq!(duration(Duration::from_millis(10)), "0.01s");
        assert_eq!(duration(Duration::from_millis(200)), "0.2s");
        assert_eq!(duration(Duration::from_millis(1200)), "1.2s");
    }
}
