#![cfg(feature = "cli")]

use async_trait::async_trait;
use rut::cli::{
    ArgSpec, CoreArgs, PluginArgs, PluginRegistry, RunnerPlugin, SuiteContext, build_command,
    harness_status,
};
use rut::{
    ReporterResult, SequentialRunner, SuiteReport, TestReporter, TestResult, TestRunner, TestSuite,
    suite,
};
use std::path::{Path, PathBuf};

suite! {
    typename = PluginCliSuite;
    name = "plugin cli suite";

    test_case(name = "passing") {
        test(name = "passes") {
            TestResult::passed()
        }
    }

    test_case(name = "failing") {
        test(name = "fails") {
            TestResult::failed("expected")
        }
    }
}

struct CustomRunner(SequentialRunner);

#[async_trait]
impl TestRunner for CustomRunner {
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

impl RunnerPlugin for CustomRunner {
    const NAME: &'static str = "custom";

    fn args() -> Vec<ArgSpec> {
        vec![ArgSpec::value("custom-rounds").help("Rounds to run")]
    }

    fn from_args(args: &PluginArgs<'_>, core: &CoreArgs) -> anyhow::Result<Self> {
        assert_eq!(args.parsed::<usize>("custom-rounds")?, Some(3));

        let mut runner = SequentialRunner::new();
        for filter in &core.filters {
            runner.add_filter(filter.clone());
        }
        Ok(CustomRunner(runner))
    }
}

/// Redeclares an argument the built-in parallel runner already owns.
struct CollidingRunner(SequentialRunner);

#[async_trait]
impl TestRunner for CollidingRunner {
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

impl RunnerPlugin for CollidingRunner {
    const NAME: &'static str = "colliding";

    fn args() -> Vec<ArgSpec> {
        vec![ArgSpec::flag("shuffle")]
    }

    fn from_args(_args: &PluginArgs<'_>, _core: &CoreArgs) -> anyhow::Result<Self> {
        Ok(CollidingRunner(SequentialRunner::new()))
    }
}

fn registry() -> PluginRegistry {
    PluginRegistry::with_builtins()
}

#[test]
fn declares_core_and_plugin_arguments() {
    let command = build_command(&registry()).unwrap();
    let longs = command
        .get_arguments()
        .filter_map(|argument| argument.get_long())
        .collect::<Vec<_>>();

    for expected in [
        "runner",
        "reporters",
        "filter",
        "fail-fast",
        "jobs",
        "shuffle",
        "junit",
        "junit-dir",
        "gtest",
        "gtest-dir",
    ] {
        assert!(longs.contains(&expected), "missing --{expected}");
    }
}

#[test]
fn rejects_arguments_declared_twice() {
    let mut registry = registry();
    registry.register_runner::<CollidingRunner>();

    let error = registry.validate().unwrap_err().to_string();

    assert!(error.contains("--shuffle"), "unexpected error: {error}");
}

#[test]
fn rejects_duplicate_plugin_names() {
    let mut registry = PluginRegistry::new();
    registry
        .register_runner::<CustomRunner>()
        .register_runner::<CustomRunner>();

    assert!(
        registry
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate runner plugin name 'custom'")
    );
}

#[test]
fn mirrors_the_suite_path_in_report_directories() {
    let suite = SuiteContext {
        typename: "CalculatorSuite".to_string(),
        slug: "calculator-suite".to_string(),
        source_file: None,
        relative_dir: PathBuf::from("nested"),
    };

    assert_eq!(
        suite.mirrored_path(Path::new("target/junit"), "xml"),
        PathBuf::from("target/junit/nested/calculator-suite.xml")
    );
}

#[tokio::test]
async fn reports_failing_tests_through_the_exit_status() {
    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        ["rut", "--runner", "sequential"],
    )
    .await;

    assert_eq!(status, 1);
}

#[tokio::test]
async fn applies_core_filters_to_the_selected_runner() {
    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        ["rut", "--runner", "sequential", "--filter", "passing"],
    )
    .await;

    assert_eq!(status, 0);
}

#[tokio::test]
async fn runs_a_plugin_supplied_runner_with_its_own_arguments() {
    let mut registry = registry();
    registry.register_runner::<CustomRunner>();

    let status = harness_status(
        registry,
        || Box::new(PluginCliSuite::new()),
        [
            "rut",
            "--runner",
            "custom",
            "--custom-rounds",
            "3",
            "--filter",
            "passing",
        ],
    )
    .await;

    assert_eq!(status, 0);
}

#[tokio::test]
async fn rejects_an_unknown_runner() {
    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        ["rut", "--runner", "nope"],
    )
    .await;

    assert_eq!(status, 2);
}

#[tokio::test]
async fn writes_reports_activated_by_their_output_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let junit = directory.path().join("report.xml");
    let gtest = directory.path().join("report.json");

    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        [
            "rut",
            "--runner",
            "sequential",
            "--filter",
            "passing",
            "--junit",
            junit.to_str().unwrap(),
            "--gtest",
            gtest.to_str().unwrap(),
        ],
    )
    .await;

    assert_eq!(status, 0);
    assert!(junit.is_file());
    assert!(gtest.is_file());
}

#[tokio::test]
async fn rejects_conflicting_report_outputs() {
    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        ["rut", "--junit", "a.xml", "--junit-dir", "reports"],
    )
    .await;

    assert_eq!(status, 2);
}

#[tokio::test]
async fn rejects_a_repeated_reporters_option() {
    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        ["rut", "--reporters=stdout", "--reporters=junit"],
    )
    .await;

    assert_eq!(status, 2);
}

#[tokio::test]
async fn accepts_a_comma_separated_reporter_list() {
    let directory = tempfile::tempdir().unwrap();
    let junit = directory.path().join("report.xml");

    let status = harness_status(
        registry(),
        || Box::new(PluginCliSuite::new()),
        [
            "rut",
            "--runner",
            "sequential",
            "--filter",
            "passing",
            "--reporters=stdout,junit",
            "--junit",
            junit.to_str().unwrap(),
        ],
    )
    .await;

    assert_eq!(status, 0);
    assert!(junit.is_file());
}
