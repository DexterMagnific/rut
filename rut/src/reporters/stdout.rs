use crate::report::{SuiteReport, TestCaseInfo, TestResult, TestStatus};
use crate::reporter::{ReporterResult, TestReporter};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

#[derive(Default)]
pub struct StdoutReporter {
    report: Option<SuiteReport>,
}

impl StdoutReporter {
    pub fn new() -> Self {
        Self { report: None }
    }

    fn report(&self) -> &SuiteReport {
        self.report.as_ref().expect("report_start not called")
    }

    fn report_mut(&mut self) -> &mut SuiteReport {
        self.report.as_mut().expect("report_start not called")
    }
}

#[async_trait]
impl TestReporter for StdoutReporter {
    async fn report_start(
        &mut self,
        suite_name: &str,
        test_cases: &[TestCaseInfo],
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        let total_tests: usize = test_cases.iter().map(|c| c.tests.len()).sum();
        self.report = Some(SuiteReport::new(suite_name, test_cases, started_at));

        println!(
            "Running test suite: {} ({} test cases, {} tests)",
            suite_name,
            test_cases.len(),
            total_tests
        );
        Ok(())
    }

    async fn report_case_start(
        &mut self,
        case_name: &str,
        test_count: usize,
        started_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        if let Some(case) = self
            .report_mut()
            .test_cases
            .iter_mut()
            .find(|case| case.name == case_name)
        {
            case.status = TestStatus::Running;
            case.started_at = Some(started_at);
        }
        println!("  Test case: {} ({} tests)", case_name, test_count);
        Ok(())
    }

    async fn report_test_start(&mut self, case_name: &str, test_name: &str) -> ReporterResult<()> {
        let source = self
            .report_mut()
            .test_cases
            .iter_mut()
            .find(|case| case.name == case_name)
            .and_then(|case| case.tests.iter_mut().find(|test| test.name == test_name))
            .and_then(|test| {
                test.status = TestStatus::Running;
                test.source
                    .as_ref()
                    .map(|source| format!("{}:{}:{}", source.file, source.line, source.column))
            });
        println!(
            "    Running: {}{}",
            test_name,
            source.map_or_else(String::new, |source| format!(" ({source})"))
        );
        Ok(())
    }

    async fn report_result(&mut self, case_name: &str, result: &TestResult) -> ReporterResult<()> {
        let report = self.report_mut();
        if let Some(case) = report
            .test_cases
            .iter_mut()
            .find(|case| case.name == case_name)
            && let Some(test) = case.tests.iter_mut().find(|test| test.name == result.name)
        {
            test.status = result.status;
            test.message = result.message.clone();
            test.source = result.source.clone();
            test.failure_location = result.failure_location.clone();
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
                TestStatus::Skipped => {
                    case.skipped += 1;
                    report.total_skipped += 1;
                }
                TestStatus::NotYetRun | TestStatus::Running => {}
            }
        }

        let status = match result.status {
            TestStatus::Passed => "PASS",
            TestStatus::Failed => "FAIL",
            TestStatus::Skipped => "SKIP",
            _ => "UNKNOWN",
        };
        let msg = result.message.as_deref().unwrap_or("");
        let failure_location = result
            .failure_location
            .as_ref()
            .map(|location| format!("{}:{}:{}: ", location.file, location.line, location.column))
            .unwrap_or_default();
        println!(
            "    {} {} (duration: {:.2?}, total duration: {:.2?}){}",
            status,
            result.name,
            result.duration,
            result.total_duration,
            if msg.is_empty() {
                String::new()
            } else {
                format!(" - {failure_location}{msg}")
            }
        );

        // Print properties
        for (key, value) in &result.properties {
            println!("      Property: {} = {}", key, value);
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
        if let Some(case) = self
            .report_mut()
            .test_cases
            .iter_mut()
            .find(|case| case.name == case_name)
        {
            case.status = if case.failed > 0 {
                TestStatus::Failed
            } else if case.passed == 0 && case.skipped > 0 {
                TestStatus::Skipped
            } else {
                TestStatus::Passed
            };
            case.finished_at = Some(finished_at);
            case.duration = duration;
            case.total_duration = total_duration;
        }
        println!(
            "  Finished: {} (duration: {:.2?}, total duration: {:.2?})",
            case_name, duration, total_duration
        );
        Ok(())
    }

    async fn report_finish(
        &mut self,
        duration: Duration,
        total_duration: Duration,
        finished_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        let report = self.report_mut();
        report.finished_at = finished_at;
        report.duration = duration;
        report.total_duration = total_duration;
        println!(
            "\nTest run completed: {} passed, {} failed, {} skipped (duration: {:.2?}, total duration: {:.2?})",
            report.total_passed,
            report.total_failed,
            report.total_skipped,
            report.duration,
            report.total_duration
        );
        Ok(())
    }

    fn get_report(&self) -> &SuiteReport {
        self.report()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::TestInfo;

    #[tokio::test]
    async fn independently_builds_a_report_from_runner_events() {
        let started_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let case_started_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:01Z")
            .unwrap()
            .with_timezone(&Utc);
        let case_finished_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:02Z")
            .unwrap()
            .with_timezone(&Utc);
        let finished_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:03Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut reporter = StdoutReporter::new();

        reporter
            .report_start(
                "suite",
                &[TestCaseInfo {
                    name: "case".to_string(),
                    tests: vec![TestInfo {
                        name: "test".to_string(),
                        source: None,
                    }],
                }],
                started_at,
            )
            .await
            .unwrap();
        reporter
            .report_case_start("case", 1, case_started_at)
            .await
            .unwrap();
        reporter.report_test_start("case", "test").await.unwrap();

        let mut result = TestResult::passed().with_property("kind", "unit");
        result.name = "test".to_string();
        result.duration = Duration::from_millis(2);
        result.total_duration = Duration::from_millis(2);
        reporter.report_result("case", &result).await.unwrap();
        reporter
            .report_case_finish(
                "case",
                Duration::from_millis(3),
                Duration::from_millis(4),
                case_finished_at,
            )
            .await
            .unwrap();
        reporter
            .report_finish(
                Duration::from_millis(5),
                Duration::from_millis(6),
                finished_at,
            )
            .await
            .unwrap();

        let report = reporter.get_report();
        assert_eq!(report.total_passed, 1);
        assert_eq!(report.started_at, started_at);
        assert_eq!(report.finished_at, finished_at);
        assert_eq!(report.test_cases[0].started_at, Some(case_started_at));
        assert_eq!(report.test_cases[0].finished_at, Some(case_finished_at));
        assert_eq!(
            report.test_cases[0].tests[0].properties,
            vec![("kind".to_string(), "unit".to_string())]
        );
        assert_eq!(report.total_duration, Duration::from_millis(6));
    }
}
