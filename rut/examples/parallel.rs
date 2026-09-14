mod support;

use std::time::Duration;

use rut::{ParallelRunnerBuilder, StdoutReporter, TestRunnerInternal};
use support::{ExampleCase, ExampleContext, ExampleSuite, ExampleTest};

fn slow_case_setup_ran(context: &ExampleContext) -> Result<(), String> {
    require_event(context, "slow case setup")
}

fn fast_case_setup_ran(context: &ExampleContext) -> Result<(), String> {
    require_event(context, "fast case setup")
}

fn require_event(context: &ExampleContext, expected: &str) -> Result<(), String> {
    context
        .events()
        .iter()
        .any(|event| event == expected)
        .then_some(())
        .ok_or_else(|| format!("expected lifecycle event: {expected}"))
}

#[tokio::main]
async fn main() {
    let suite = ExampleSuite::new(
        "parallel example",
        vec![
            Box::new(ExampleCase::new(
                "slow case",
                vec![ExampleTest::delayed(
                    "slow test",
                    Duration::from_millis(100),
                    slow_case_setup_ran,
                )],
            )),
            Box::new(ExampleCase::new(
                "fast case",
                vec![ExampleTest::delayed(
                    "fast test",
                    Duration::from_millis(10),
                    fast_case_setup_ran,
                )],
            )),
        ],
    );

    let observed_context = suite.example_context().clone();
    let report = ParallelRunnerBuilder::new()
        .with_max_jobs(2)
        .shuffle_test_cases()
        .with_suite(Box::new(suite))
        .with_reporter(Box::new(StdoutReporter::new()))
        .build()
        .run()
        .await;

    assert_eq!(report.total_passed, 2);
    assert_eq!(report.total_failed, 0);

    let events = observed_context.events();
    let fast_finished = events
        .iter()
        .position(|event| event == "fast case teardown")
        .expect("fast case should finish");
    let slow_finished = events
        .iter()
        .position(|event| event == "slow case teardown")
        .expect("slow case should finish");
    assert!(fast_finished < slow_finished);

    println!("\nLifecycle events (cases may interleave):");
    for event in events {
        println!("  {event}");
    }
}
