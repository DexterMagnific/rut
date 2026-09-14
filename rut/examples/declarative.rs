use std::sync::{Arc, Mutex};

use rut::{SequentialRunner, StdoutReporter, TestResult, TestRunnerInternal, suite};

#[derive(Clone, Default)]
struct AppContext {
    events: Arc<Mutex<Vec<String>>>,
}

impl AppContext {
    fn record(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }
}

suite! {
    typename = DslSuite;
    name = "Suite using the DSL";

    fn record(context: &AppContext, event: &str) {
        context.record(event);
    }

    context = AppContext;

    setup {
        tokio::task::yield_now().await;
        let app = AppContext::default();
        record(&app, "suite setup");
        context.set(app);
    }

    test_case(name = "arithmetic") {
        fn record_operation(context: &AppContext, operation: &str) {
            record(context, &format!("{operation} ran"));
        }

        setup {
            record(context, "arithmetic setup");
        }

        test(
            name = "two plus two",
            category = "arithmetic",
            expression = "2 + 2"
        ) {
            tokio::task::yield_now().await;
            record_operation(context, "addition");
            TestResult::passed()
                .with_property("category", "runtime")
        }

        test(name = "multiplication") {
            record_operation(context, "multiplication");
            TestResult::passed()
        }

        teardown {
            record(context, "arithmetic teardown");
        }
    }

    teardown {
        record(context, "suite teardown");
    }

}

#[tokio::main]
async fn main() {
    let report = SequentialRunner::new()
        .with_suite(Box::new(DslSuite::new()))
        .with_reporter(Box::new(StdoutReporter::new()))
        .run()
        .await;

    assert_eq!(report.total_passed, 2);
    assert_eq!(report.total_failed, 0);
    assert_eq!(
        report.test_cases[0].tests[0].properties,
        vec![
            ("category".to_owned(), "runtime".to_owned()),
            ("category".to_owned(), "arithmetic".to_owned()),
            ("expression".to_owned(), "2 + 2".to_owned()),
        ]
    );
}
