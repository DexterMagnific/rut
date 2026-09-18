//! Example plugin crate used to exercise `cargo rut run --plugin-crate`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rut::cli::{ArgSpec, CoreArgs, PluginArgs, ReporterPlugin, RunnerPlugin, SuiteContext};
use rut::{
    ReporterResult, SequentialRunner, SuiteReport, TestCaseInfo, TestReporter, TestResult,
    TestRunner, TestStatus, TestSuite,
};
use std::time::Duration;

/// Runs cases sequentially, announcing a configurable repeat count.
pub struct RepeatRunner(SequentialRunner);

impl RepeatRunner {
    pub fn new(passes: usize) -> Self {
        println!("repeat runner: {passes} pass(es)");
        Self(SequentialRunner::new())
    }

    /// Applies the runner-agnostic command line options.
    pub fn apply(&mut self, core: &CoreArgs) {
        for filter in &core.filters {
            self.0.add_filter(filter.clone());
        }
        self.0.set_fail_fast(core.fail_fast);
    }
}

#[async_trait]
impl TestRunner for RepeatRunner {
    fn set_suite(&mut self, suite: Box<dyn TestSuite>) {
        self.0.set_suite(suite);
    }

    fn set_reporter(&mut self, reporter: Box<dyn TestReporter>) {
        self.0.set_reporter(reporter);
    }

    async fn run(self) -> ReporterResult<SuiteReport> {
        self.0.run().await
    }
}

impl RunnerPlugin for RepeatRunner {
    const NAME: &'static str = "repeat";
    const ABOUT: &'static str = "Runs the suite sequentially, announcing a repeat count";

    fn args() -> Vec<ArgSpec> {
        vec![
            ArgSpec::value("repeat-count")
                .value_name("N")
                .default("1")
                .help("Number of times the suite is announced"),
        ]
    }

    fn from_args(args: &PluginArgs<'_>, core: &CoreArgs) -> anyhow::Result<Self> {
        let mut runner = RepeatRunner::new(args.parsed("repeat-count")?.unwrap_or(1));
        runner.apply(core);
        Ok(runner)
    }
}

/// Prints one line per finished suite.
pub struct SummaryReporter {
    report: SuiteReport,
    label: String,
}

impl SummaryReporter {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            report: empty_report(),
            label: label.into(),
        }
    }
}

/// Default prefix of the summary line, also used by the co-located suite.
pub fn summary_label() -> &'static str {
    "summary"
}

impl ReporterPlugin for SummaryReporter {
    const NAME: &'static str = "summary";
    const ABOUT: &'static str = "Prints a single summary line";

    fn args() -> Vec<ArgSpec> {
        vec![
            ArgSpec::value("summary-label")
                .value_name("TEXT")
                .default("summary")
                .help("Prefix of the summary line"),
        ]
    }

    fn from_args(args: &PluginArgs<'_>, _suite: &SuiteContext) -> anyhow::Result<Self> {
        Ok(SummaryReporter::new(
            args.value("summary-label").unwrap_or(summary_label()),
        ))
    }
}

fn empty_report() -> SuiteReport {
    SuiteReport {
        suite_name: String::new(),
        test_cases: Vec::new(),
        total_passed: 0,
        total_failed: 0,
        total_skipped: 0,
        duration: Duration::ZERO,
        total_duration: Duration::ZERO,
        started_at: Utc::now(),
        finished_at: Utc::now(),
    }
}

#[async_trait]
impl TestReporter for SummaryReporter {
    async fn report_start(
        &mut self,
        suite_name: &str,
        _test_cases: &[TestCaseInfo],
        _started_at: DateTime<Utc>,
    ) -> ReporterResult<()> {
        self.report.suite_name = suite_name.to_string();
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

    async fn report_test_start(&mut self, _case: &str, _test: &str) -> ReporterResult<()> {
        Ok(())
    }

    async fn report_result(&mut self, _case_name: &str, result: &TestResult) -> ReporterResult<()> {
        match result.status {
            TestStatus::Passed => self.report.total_passed += 1,
            TestStatus::Skipped => self.report.total_skipped += 1,
            _ => self.report.total_failed += 1,
        }
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
        println!(
            "{}: {} passed, {} failed, {} skipped",
            self.label,
            self.report.total_passed,
            self.report.total_failed,
            self.report.total_skipped
        );
        Ok(())
    }

    fn get_report(&self) -> &SuiteReport {
        &self.report
    }
}

rut::export_plugins! {
    runners: [RepeatRunner],
    reporters: [SummaryReporter],
}
