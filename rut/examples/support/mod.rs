use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rut::{Test, TestCase, TestContext, TestResult, TestSuite};

#[derive(Clone, Default)]
pub struct ExampleContext {
    events: Arc<Mutex<Vec<String>>>,
}

impl ExampleContext {
    pub fn record(&self, event: impl Into<String>) {
        self.events.lock().unwrap().push(event.into());
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

#[derive(Clone)]
pub struct ExampleSuite {
    name: &'static str,
    context: Option<TestContext>,
    observed_context: ExampleContext,
    test_cases: Vec<Box<dyn TestCase>>,
}

impl ExampleSuite {
    pub fn new(name: &'static str, test_cases: Vec<Box<dyn TestCase>>) -> Self {
        Self {
            name,
            context: None,
            observed_context: ExampleContext::default(),
            test_cases,
        }
    }

    pub fn example_context(&self) -> &ExampleContext {
        &self.observed_context
    }
}

#[async_trait]
impl TestSuite for ExampleSuite {
    async fn setup_suite(&mut self) {
        self.observed_context.record("suite setup");
        self.context = Some(TestContext::new(self.observed_context.clone()));
    }

    fn context(&self) -> Option<&TestContext> {
        self.context.as_ref()
    }

    fn context_mut(&mut self) -> &mut Option<TestContext> {
        &mut self.context
    }

    async fn teardown_suite(&mut self) {
        self.example_context().record("suite teardown");
    }

    fn name(&self) -> &str {
        self.name
    }

    fn test_cases(&self) -> Vec<Box<dyn TestCase>> {
        self.test_cases.clone()
    }

    fn test_cases_mut(&mut self) -> &mut Vec<Box<dyn TestCase>> {
        &mut self.test_cases
    }
}

#[derive(Clone)]
pub struct ExampleCase {
    name: &'static str,
    tests: Vec<ExampleTest>,
}

impl ExampleCase {
    pub fn new(name: &'static str, tests: Vec<ExampleTest>) -> Self {
        Self { name, tests }
    }
}

#[async_trait]
impl TestCase for ExampleCase {
    fn name(&self) -> &str {
        self.name
    }

    async fn setup_case(&mut self, context: Option<&TestContext>) {
        example_context(context).record(format!("{} setup", self.name));
    }

    async fn teardown_case(&mut self, context: Option<&TestContext>) {
        example_context(context).record(format!("{} teardown", self.name));
    }

    fn tests(&self) -> Vec<Box<dyn Test>> {
        self.tests
            .iter()
            .cloned()
            .map(|test| Box::new(test) as Box<dyn Test>)
            .collect()
    }

    fn clone_box(&self) -> Box<dyn TestCase> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct ExampleTest {
    name: &'static str,
    delay: Duration,
    check: fn(&ExampleContext) -> Result<(), String>,
}

impl ExampleTest {
    #[allow(dead_code)]
    pub fn new(name: &'static str, check: fn(&ExampleContext) -> Result<(), String>) -> Self {
        Self {
            name,
            delay: Duration::ZERO,
            check,
        }
    }

    #[allow(dead_code)]
    pub fn delayed(
        name: &'static str,
        delay: Duration,
        check: fn(&ExampleContext) -> Result<(), String>,
    ) -> Self {
        Self { name, delay, check }
    }
}

#[async_trait]
impl Test for ExampleTest {
    fn name(&self) -> &str {
        self.name
    }

    async fn run(&self, context: Option<&TestContext>) -> TestResult {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }

        let context = example_context(context);
        context.record(format!("{} ran", self.name));

        let mut result = match (self.check)(context) {
            Ok(()) => TestResult::passed(),
            Err(message) => TestResult::failed(message),
        };
        result.name = self.name.to_owned();

        result.with_property("delay_ms", self.delay.as_millis().to_string())
    }
}

fn example_context(context: Option<&TestContext>) -> &ExampleContext {
    context
        .expect("test suite did not initialize its context")
        .downcast_ref::<ExampleContext>()
        .expect("test uses ExampleContext")
}
