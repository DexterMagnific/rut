use crate::report::{SuiteReport, TestCaseInfo, TestResult, TestStatus};
use crate::reporter::{ReporterResult, TestReporter};
use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::path::PathBuf;
use std::time::Duration;

pub struct JUnitReporter {
    path: PathBuf,
    report: Option<SuiteReport>,
}

impl JUnitReporter {
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
        let xml = serialize_report(self.report())?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create report directory {}", parent.display())
            })?;
        }
        std::fs::write(&self.path, xml)
            .with_context(|| format!("failed to write JUnit report to {}", self.path.display()))
    }
}

#[cfg(feature = "cli")]
impl crate::cli::ReporterPlugin for JUnitReporter {
    const NAME: &'static str = "junit";
    const ABOUT: &'static str = "Writes a JUnit XML report";

    fn args() -> Vec<crate::cli::ArgSpec> {
        crate::cli::builtin::output_args(Self::NAME, "JUnit XML")
    }

    fn is_active(args: &crate::cli::PluginArgs<'_>) -> bool {
        args.is_present(Self::NAME) || args.is_present("junit-dir")
    }

    fn from_args(
        args: &crate::cli::PluginArgs<'_>,
        suite: &crate::cli::SuiteContext,
    ) -> anyhow::Result<Self> {
        Ok(JUnitReporter::new(crate::cli::builtin::output_path(
            args,
            suite,
            Self::NAME,
            "xml",
        )?))
    }
}

#[async_trait]
impl TestReporter for JUnitReporter {
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
        case_name: &str,
        _test_count: usize,
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
        Ok(())
    }

    async fn report_test_start(&mut self, case_name: &str, test_name: &str) -> ReporterResult<()> {
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
            test.failed_attempts = result.failed_attempts;

            match result.status {
                TestStatus::Passed | TestStatus::Unstable => {
                    case.passed += 1;
                    report.total_passed += 1;
                }
                TestStatus::Failed | TestStatus::TimedOut => {
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
        self.write_report()
    }

    fn get_report(&self) -> &SuiteReport {
        self.report()
    }
}

fn serialize_report(report: &SuiteReport) -> ReporterResult<Vec<u8>> {
    let tests = report
        .test_cases
        .iter()
        .map(|case| case.tests.len())
        .sum::<usize>();
    let skipped = report
        .test_cases
        .iter()
        .flat_map(|case| &case.tests)
        .filter(|test| {
            matches!(
                test.status,
                TestStatus::Skipped | TestStatus::NotYetRun | TestStatus::Running
            )
        })
        .count();
    let tests = tests.to_string();
    let failures = report.total_failed.to_string();
    let skipped = skipped.to_string();
    let time = seconds(report.total_duration);
    let timestamp = report.started_at.to_rfc3339();

    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    write_event(
        &mut writer,
        Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)),
    )?;

    let mut root = BytesStart::new("testsuites");
    root.push_attribute(("tests", tests.as_str()));
    root.push_attribute(("failures", failures.as_str()));
    root.push_attribute(("errors", "0"));
    root.push_attribute(("skipped", skipped.as_str()));
    root.push_attribute(("time", time.as_str()));
    write_event(&mut writer, Event::Start(root))?;

    let mut suite = BytesStart::new("testsuite");
    suite.push_attribute(("name", report.suite_name.as_str()));
    suite.push_attribute(("tests", tests.as_str()));
    suite.push_attribute(("failures", failures.as_str()));
    suite.push_attribute(("errors", "0"));
    suite.push_attribute(("skipped", skipped.as_str()));
    suite.push_attribute(("time", time.as_str()));
    suite.push_attribute(("timestamp", timestamp.as_str()));
    write_event(&mut writer, Event::Start(suite))?;

    for case in &report.test_cases {
        for test in &case.tests {
            let test_time = seconds(test.total_duration);
            let source_line = test.source.as_ref().map(|source| source.line.to_string());
            let source_column = test.source.as_ref().map(|source| source.column.to_string());
            let mut testcase = BytesStart::new("testcase");
            testcase.push_attribute(("name", test.name.as_str()));
            testcase.push_attribute(("classname", case.name.as_str()));
            testcase.push_attribute(("time", test_time.as_str()));
            if let Some(source) = &test.source {
                testcase.push_attribute(("file", source.file.as_str()));
            }
            if let Some(line) = &source_line {
                testcase.push_attribute(("line", line.as_str()));
            }
            if let Some(column) = &source_column {
                testcase.push_attribute(("column", column.as_str()));
            }
            write_event(&mut writer, Event::Start(testcase))?;

            if !test.properties.is_empty() {
                write_event(&mut writer, Event::Start(BytesStart::new("properties")))?;
                for (name, value) in &test.properties {
                    let mut property = BytesStart::new("property");
                    property.push_attribute(("name", name.as_str()));
                    property.push_attribute(("value", value.as_str()));
                    write_event(&mut writer, Event::Empty(property))?;
                }
                write_event(&mut writer, Event::End(BytesEnd::new("properties")))?;
            }

            match test.status {
                TestStatus::Failed | TestStatus::TimedOut => {
                    let message = failure_message(test);
                    let mut failure = BytesStart::new("failure");
                    failure.push_attribute(("message", message.as_str()));
                    write_event(&mut writer, Event::Start(failure))?;
                    write_event(&mut writer, Event::Text(BytesText::new(&message)))?;
                    write_event(&mut writer, Event::End(BytesEnd::new("failure")))?;
                }
                TestStatus::Skipped => {
                    let mut skipped = BytesStart::new("skipped");
                    if let Some(reason) = &test.message {
                        skipped.push_attribute(("message", reason.as_str()));
                    }
                    write_event(&mut writer, Event::Empty(skipped))?;
                }
                TestStatus::NotYetRun | TestStatus::Running => {
                    write_event(&mut writer, Event::Empty(BytesStart::new("skipped")))?;
                }
                TestStatus::Passed | TestStatus::Unstable => {}
            }

            write_event(&mut writer, Event::End(BytesEnd::new("testcase")))?;
        }
    }

    write_event(&mut writer, Event::End(BytesEnd::new("testsuite")))?;
    write_event(&mut writer, Event::End(BytesEnd::new("testsuites")))?;
    Ok(writer.into_inner())
}

fn failure_message(test: &TestResult) -> String {
    let message = test.message.as_deref().unwrap_or("test failed");
    test.failure_location.as_ref().map_or_else(
        || message.to_string(),
        |location| {
            format!(
                "{}:{}:{}: {message}",
                location.file, location.line, location.column
            )
        },
    )
}

fn write_event(writer: &mut Writer<Vec<u8>>, event: Event<'_>) -> ReporterResult<()> {
    writer
        .write_event(event)
        .context("failed to serialize JUnit report")
}

fn seconds(duration: Duration) -> String {
    format!("{:.6}", duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{SourceLocation, TestInfo};
    use quick_xml::Reader;
    use quick_xml::events::Event;
    use tempfile::TempDir;

    #[test]
    fn writes_explicit_skip_reasons() {
        let started_at = Utc::now();
        let mut report = SuiteReport::new(
            "suite",
            &[TestCaseInfo {
                name: "case".to_string(),
                tests: vec![TestInfo {
                    name: "test".to_string(),
                    source: None,
                }],
            }],
            started_at,
        );
        report.test_cases[0].tests[0] = TestResult::skipped("requires database");
        report.test_cases[0].tests[0].name = "test".to_string();

        let xml = String::from_utf8(serialize_report(&report).unwrap()).unwrap();

        assert!(xml.contains("<skipped message=\"requires database\"/>"));
    }

    #[tokio::test]
    async fn writes_valid_xml_with_failures_skips_and_test_properties() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("nested/report.xml");
        let mut reporter = JUnitReporter::new(&path);
        let suite_started_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let case_started_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:01Z")
            .unwrap()
            .with_timezone(&Utc);
        let case_finished_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:02Z")
            .unwrap()
            .with_timezone(&Utc);
        let suite_finished_at = DateTime::parse_from_rfc3339("2026-09-08T10:00:03Z")
            .unwrap()
            .with_timezone(&Utc);
        reporter
            .report_start(
                "A & B",
                &[TestCaseInfo {
                    name: "case <one>".to_string(),
                    tests: vec![
                        TestInfo {
                            name: "passes".to_string(),
                            source: Some(SourceLocation::new("tests/example.rs", 10, 9)),
                        },
                        TestInfo {
                            name: "fails".to_string(),
                            source: Some(SourceLocation::new("tests/example.rs", 20, 9)),
                        },
                        TestInfo {
                            name: "later".to_string(),
                            source: None,
                        },
                    ],
                }],
                suite_started_at,
            )
            .await
            .unwrap();
        reporter
            .report_case_start("case <one>", 3, case_started_at)
            .await
            .unwrap();
        reporter
            .report_test_start("case <one>", "passes")
            .await
            .unwrap();
        let mut passed = TestResult::passed().with_property("key & one", "value <one>");
        passed.name = "passes".to_string();
        passed.source = Some(SourceLocation::new("tests/example.rs", 10, 9));
        reporter.report_result("case <one>", &passed).await.unwrap();
        reporter
            .report_test_start("case <one>", "fails")
            .await
            .unwrap();
        let mut failed = TestResult::failed("expected <x> & got y");
        failed.name = "fails".to_string();
        failed.source = Some(SourceLocation::new("tests/example.rs", 20, 9));
        failed.failure_location = Some(SourceLocation::new("tests/example.rs", 24, 13));
        reporter.report_result("case <one>", &failed).await.unwrap();
        reporter
            .report_case_finish(
                "case <one>",
                Duration::from_millis(2),
                Duration::from_millis(3),
                case_finished_at,
            )
            .await
            .unwrap();
        reporter
            .report_finish(
                Duration::from_millis(3),
                Duration::from_millis(4),
                suite_finished_at,
            )
            .await
            .unwrap();

        let xml = std::fs::read_to_string(path).unwrap();
        let mut reader = Reader::from_str(&xml);
        let mut starts = Vec::new();
        loop {
            match reader.read_event().unwrap() {
                Event::Start(event) | Event::Empty(event) => {
                    starts.push(String::from_utf8_lossy(event.name().as_ref()).into_owned());
                }
                Event::Eof => break,
                _ => {}
            }
        }

        assert!(starts.contains(&"testsuites".to_string()));
        assert_eq!(starts.iter().filter(|name| *name == "testcase").count(), 3);
        assert!(starts.contains(&"failure".to_string()));
        assert!(starts.contains(&"skipped".to_string()));
        assert!(xml.contains("<properties>"));
        assert!(xml.contains("name=\"key &amp; one\" value=\"value &lt;one&gt;\""));
        assert!(xml.contains("file=\"tests/example.rs\" line=\"10\" column=\"9\""));
        assert!(xml.contains("file=\"tests/example.rs\" line=\"20\" column=\"9\""));
        assert!(xml.contains("tests/example.rs:24:13: expected &lt;x&gt; &amp; got y"));
        assert!(xml.contains("timestamp=\"2026-09-08T10:00:00+00:00\""));
        assert_eq!(reporter.get_report().started_at, suite_started_at);
        assert_eq!(reporter.get_report().finished_at, suite_finished_at);
        assert_eq!(
            reporter.get_report().test_cases[0].started_at,
            Some(case_started_at)
        );
        assert_eq!(
            reporter.get_report().test_cases[0].finished_at,
            Some(case_finished_at)
        );
    }

    #[tokio::test]
    async fn reports_an_invalid_destination() {
        let directory = TempDir::new().unwrap();
        let blocking_file = directory.path().join("not-a-directory");
        std::fs::write(&blocking_file, "content").unwrap();
        let path = blocking_file.join("report.xml");
        let mut reporter = JUnitReporter::new(&path);
        reporter
            .report_start("suite", &[], Utc::now())
            .await
            .unwrap();

        let error = reporter
            .report_finish(Duration::ZERO, Duration::ZERO, Utc::now())
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to create report directory")
        );
        assert!(error.to_string().contains("not-a-directory"));
        assert!(error.downcast_ref::<std::io::Error>().is_some());
    }
}
