mod support;

use rut::{SequentialRunner, StdoutReporter, TestRunnerInternal};
use support::{ExampleCase, ExampleContext, ExampleSuite, ExampleTest};

fn suite_setup_ran(context: &ExampleContext) -> Result<(), String> {
    require_event(context, "suite setup")
}

fn numbers_setup_ran(context: &ExampleContext) -> Result<(), String> {
    require_event(context, "numbers setup")
}

fn strings_setup_ran(context: &ExampleContext) -> Result<(), String> {
    require_event(context, "strings setup")
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
        "sequential example",
        vec![
            Box::new(ExampleCase::new(
                "numbers",
                vec![
                    ExampleTest::new("suite setup is visible", suite_setup_ran),
                    ExampleTest::new("case setup is visible", numbers_setup_ran),
                ],
            )),
            Box::new(ExampleCase::new(
                "strings",
                vec![ExampleTest::new(
                    "second case setup is visible",
                    strings_setup_ran,
                )],
            )),
        ],
    );

    let observed_context = suite.example_context().clone();
    let report = SequentialRunner::new()
        .with_suite(Box::new(suite))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await;

    assert_eq!(report.total_passed, 3);
    assert_eq!(report.total_failed, 0);

    println!("\nLifecycle events:");
    for event in observed_context.events() {
        println!("  {event}");
    }
}
