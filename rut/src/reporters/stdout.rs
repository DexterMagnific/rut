use crate::report::{BoxFuture, SuiteReport, TestCaseInfo, TestResult, TestStatus};
use crate::reporter::TestReporterInternal;
use std::time::Duration;

#[derive(Default)]
pub struct StdoutReporter {
    report: Option<SuiteReport>,
}

impl StdoutReporter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TestReporterInternal for StdoutReporter {
    fn report_start<'a>(
        &'a mut self,
        suite_name: &'a str,
        test_cases: &'a [TestCaseInfo],
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            let total_tests: usize = test_cases.iter().map(|c| c.test_names.len()).sum();
            self.report = Some(SuiteReport::new(suite_name, test_cases));

            println!(
                "Running test suite: {} ({} test cases, {} tests)",
                suite_name,
                test_cases.len(),
                total_tests
            );
        })
    }

    fn report_case_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_count: usize,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(report) = &mut self.report {
                if let Some(case) = report.test_cases.iter_mut().find(|c| c.name == case_name) {
                    case.status = TestStatus::Running;
                    case.started_at = Some(chrono::Utc::now());
                }
            }
            println!("  Test case: {} ({} tests)", case_name, test_count);
        })
    }

    fn report_test_start<'a>(
        &'a mut self,
        case_name: &'a str,
        test_name: &'a str,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(report) = &mut self.report {
                if let Some(case) = report.test_cases.iter_mut().find(|c| c.name == case_name) {
                    if let Some(test) = case.tests.iter_mut().find(|t| t.name == test_name) {
                        test.status = TestStatus::Running;
                    }
                }
            }
            println!("    Running: {}", test_name);
        })
    }

    fn report_result<'a>(
        &'a mut self,
        case_name: &'a str,
        result: &'a TestResult,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(report) = &mut self.report {
                if let Some(case) = report.test_cases.iter_mut().find(|c| c.name == case_name) {
                    if let Some(test) = case.tests.iter_mut().find(|t| t.name == result.name) {
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
                            _ => {}
                        }
                    }
                }
            }

            let status = match result.status {
                TestStatus::Passed => "PASS",
                TestStatus::Failed => "FAIL",
                _ => "UNKNOWN",
            };
            let msg = result.message.as_deref().unwrap_or("");
            println!(
                "    {} {} (duration: {:.2?}, total duration: {:.2?}){}",
                status,
                result.name,
                result.duration,
                result.total_duration,
                if msg.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", msg)
                }
            );

            // Print properties
            for (key, value) in &result.properties {
                println!("      Property: {} = {}", key, value);
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
            if let Some(report) = &mut self.report {
                if let Some(case) = report.test_cases.iter_mut().find(|c| c.name == case_name) {
                    case.status = if case.failed > 0 {
                        TestStatus::Failed
                    } else {
                        TestStatus::Passed
                    };
                    case.finished_at = Some(chrono::Utc::now());
                    case.duration = duration;
                    case.total_duration = total_duration;
                }
            }
            println!(
                "  Finished: {} (duration: {:.2?}, total duration: {:.2?})",
                case_name, duration, total_duration
            );
        })
    }

    fn report_finish<'a>(
        &'a mut self,
        duration: Duration,
        total_duration: Duration,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(report) = &mut self.report {
                report.finished_at = chrono::Utc::now();
                report.duration = duration;
                report.total_duration = total_duration;

                println!(
                    "\nTest run completed: {} passed, {} failed (duration: {:.2?}, total duration: {:.2?})",
                    report.total_passed,
                    report.total_failed,
                    report.duration,
                    report.total_duration
                );
            }
        })
    }

    fn get_report(&self) -> &SuiteReport {
        self.report.as_ref().expect("report_start not called")
    }
}
